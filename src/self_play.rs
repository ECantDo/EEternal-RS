//! Self-play training data for the NNUE, written as viriformat (what bullet's `ViriBinpackLoader` reads).
//! Only compiled with `--features data_gen`. Run it from UCI:
//!
//!     datagen <threads> <nodes> <games per thread> <output prefix> [self | adversarial]
//!     datagen 20 5000 10000 data/run1      -> data/run1_0.vf ... data/run1_19.vf
//!
//! `adversarial`: every game is the big (1024) net against the small (512) one, each with its own search state and
//! hash, colours swapped every game; each move is recorded with the eval of the side that played it. Builds with the
//! `datagen_adversarial` feature default to it, so the datagen client (which doesn't pass a mode) can run either.
//!
//! A viriformat game is the starting position plus every move played with the eval of the position it
//! was played from (white-relative), and the result. Every move has to be recorded, so nothing is
//! skipped here: in check / captures / extreme evals get filtered when training instead (bullet's
//! viriformat `Filter`). The files can just be concatenated (`cat`) to merge them.
//!
//! Ported from chessEngine's `generateTrainingData` (self_play_training.cpp).

use std::{
	fs::File,
	io::{BufWriter, Write},
	sync::{Arc, atomic::Ordering},
	time::{SystemTime, UNIX_EPOCH},
};

use viriformat::{
	chess::{
		board::{Board as ViriBoard, DrawType, GameOutcome, WinType},
		chessmove::{Move as ViriMove, MoveFlags},
		piece::PieceType as ViriPieceType,
		types::Square as ViriSquare,
	},
	dataformat::Game,
};

use crate::{
	board::{Board, generate_moves::AllMoves},
	nnue::NetSize,
	search::{
		NoOutput, start_search,
		search_types::{SearchData, SharedData},
	},
	time_manager::{Limits, TimeManager},
	types::{castling::castling_rook_squares, color::Color, moves::Move, score::Score, square::Square},
};

const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

// TODO: tune these (chessEngine's values for now)
const OPENING_PLIES_MIN: u64 = 8;
const OPENING_PLIES_MAX: u64 = 10;
const OPENING_MAX_EVAL: i32 = 500; // throw the opening away if it's this lopsided
const OPENING_CHECK_NODES: u64 = 10000; // double check that the opening isn't horrible
const MAX_PLIES: usize = 256; // Games longer than this are less good
const WIN_ADJ_EVAL: i32 = 2000; // |eval| at least this ...
const WIN_ADJ_PLIES: u32 = 6; // ... this many plies in a row: win for that side
const DRAW_ADJ_MIN_PLY: usize = 40; // after move 40 ...
const DRAW_ADJ_EVAL: i32 = 15; // ... |eval| at most this ...
const DRAW_ADJ_PLIES: u32 = 8; // ... this many plies in a row: draw
const HASH_MB: usize = 16;

/// Who plays the games
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pairing {
	/// One search state and net for both sides (the net `set_net` picks)
	SelfPlay,
	/// The big net against the small net, each with its own search state and hash
	BigVsSmall,
}

impl Pairing {
	/// `datagen_adversarial` builds default to big vs small (the client runs `datagen` without a mode)
	const DEFAULT: Pairing = if cfg!(feature = "datagen_adversarial") { Pairing::BigVsSmall } else { Pairing::SelfPlay };

	fn parse(s: &str) -> Option<Self> {
		match s {
			"self" => Some(Pairing::SelfPlay),
			"adversarial" => Some(Pairing::BigVsSmall),
			_ => None,
		}
	}
}

/// xorshift64*: good enough for picking opening moves, and no `rand` dependency
struct Rng(u64);

impl Rng {
	fn new(seed: u64) -> Self {
		Self(seed | 1)
	}

	fn next(&mut self) -> u64 {
		self.0 ^= self.0 >> 12;
		self.0 ^= self.0 << 25;
		self.0 ^= self.0 >> 27;
		self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
	}

	/// 0..n
	fn below(&mut self, n: u64) -> u64 {
		self.next() % n
	}
}

/// Converts an engine move to viriformat's encoding. `board` is the position *before* the move.
/// Viriformat writes castling as "king takes its own rook" (to = the rook's square, not g1/c1).
pub fn move_to_viri(mv: Move, board: &Board) -> ViriMove {
	let sq = |s: Square| ViriSquare::new(s as u8).expect("square out of range");
	let from = sq(mv.from());

	if mv.is_castling() {
		let (rook_from, _) = castling_rook_squares(board.side_to_move(), mv.flag());
		ViriMove::new_with_flags(from, sq(rook_from), MoveFlags::Castle)
	} else if mv.is_en_passant() {
		ViriMove::new_with_flags(from, sq(mv.to()), MoveFlags::EnPassant)
	} else if mv.is_promotion() {
		// Same order on both sides: Pawn, Knight, Bishop, Rook, Queen
		let piece = ViriPieceType::new(mv.promotion_piece_type() as u8).expect("bad promotion piece");
		ViriMove::new_with_promo(from, sq(mv.to()), piece)
	} else {
		ViriMove::new(from, sq(mv.to()))
	}
}

fn legal_moves(board: &Board) -> Vec<Move> {
	(&board.generate_all_legal_moves::<AllMoves>()).into_iter().map(|entry| entry.mv()).collect()
}

/// A fixed-node search with no output, like `bench`. Returns the best move and its score from the
/// side to move's point of view.
fn search(search_data: &mut SearchData, board: &Board, nodes: u64) -> (Move, i32) {
	search_data.shared_data.nodes.reset();
	search_data.shared_data.stop.store(false, Ordering::Relaxed);
	search_data.set_board(board);
	search_data.new_search();
	search_data.time_manager = TimeManager::new(Limits::Nodes(nodes), board.full_move_number(), 0);

	let mv = start_search::<NoOutput>(search_data);
	(mv, search_data.root_move.score)
}

const fn white_relative(score: i32, stm: Color) -> i32 {
	match stm {
		Color::White => score,
		Color::Black => -score,
	}
}

/// Startpos plus a few random moves, played on both boards (the engine's and viriformat's).
/// `None` if a random move ran into mate/stalemate or the position came out too lopsided.
fn random_opening(rng: &mut Rng, search_data: &mut SearchData) -> Option<(Board, ViriBoard)> {
	let mut board = Board::startpos();
	let mut viri = ViriBoard::new();
	viri.set_from_fen(STARTPOS, false).expect("startpos");

	let plies = OPENING_PLIES_MIN + rng.below(OPENING_PLIES_MAX - OPENING_PLIES_MIN + 1);
	for _ in 0..plies {
		let moves = legal_moves(&board);
		if moves.is_empty() {
			return None;
		}
		let mv = moves[rng.below(moves.len() as u64) as usize];
		// Doubles as a check on move_to_viri: viriformat refuses an illegal move
		assert!(viri.make_move_simple(move_to_viri(mv, &board)), "viriformat rejected {}", mv.to_uci());
		board.make_move(mv);
	}

	if legal_moves(&board).is_empty() {
		return None;
	}
	let (_, score) = search(search_data, &board, OPENING_CHECK_NODES);
	(score.abs() <= OPENING_MAX_EVAL).then_some((board, viri))
}

/// Plays one game from `board` and records every move with its white-relative eval.
/// `search` plays the side to move (one search state, or the one whose turn it is).
fn play_game(mut board: Board, start: &ViriBoard, mut search: impl FnMut(&Board) -> (Move, i32)) -> Game {
	let mut game = Game::new(start);
	let mut win_streak = 0;
	let mut draw_streak = 0;

	let outcome = loop {
		if legal_moves(&board).is_empty() {
			break if !board.in_check() {
				GameOutcome::Draw(DrawType::Stalemate)
			} else if board.side_to_move() == Color::White {
				GameOutcome::BlackWin(WinType::Mate)
			} else {
				GameOutcome::WhiteWin(WinType::Mate)
			};
		}
		// TODO: is_draw also covers insufficient material and repetition; split them if you want the exact type
		if board.is_draw() {
			break GameOutcome::Draw(DrawType::FiftyMoves);
		}
		if game.len() >= MAX_PLIES {
			break GameOutcome::Draw(DrawType::Adjudication);
		}

		let stm = board.side_to_move();
		let (mv, score) = search(&board);
		if mv.is_none() {
			break GameOutcome::Draw(DrawType::Adjudication); // shouldn't happen with legal moves left
		}
		let white_score = white_relative(score, stm);

		// Mate scores don't fit an i16 eval; clamp them, the training filter (max_eval) drops them
		game.add_move(move_to_viri(mv, &board), white_score.clamp(-32000, 32000) as i16);
		board.make_move(mv);

		// Adjudication (skip it on mate scores: those games get to play out the mate)
		let mate = score.abs() >= Score::MATE_IN_MAX;
		win_streak = if !mate && white_score.abs() >= WIN_ADJ_EVAL { win_streak + 1 } else { 0 };
		draw_streak = if game.len() >= DRAW_ADJ_MIN_PLY && white_score.abs() <= DRAW_ADJ_EVAL {
			draw_streak + 1
		} else {
			0
		};

		if win_streak >= WIN_ADJ_PLIES {
			break if white_score > 0 {
				GameOutcome::WhiteWin(WinType::Adjudication)
			} else {
				GameOutcome::BlackWin(WinType::Adjudication)
			};
		}
		if draw_streak >= DRAW_ADJ_PLIES {
			break GameOutcome::Draw(DrawType::Adjudication);
		}
	};

	game.set_outcome(outcome);
	game
}

/// One worker: its own TT/search state (the node counter lives in SharedData, so it can't be shared
/// between threads) and its own output file.
/// A worker's search state: its own TT/history (cleared between games), on a pinned net or the global one.
fn searcher(net: Option<NetSize>) -> SearchData {
	let mut shared_data = SharedData::new();
	shared_data.tt.resize(HASH_MB);
	let mut search_data = SearchData::new(Arc::new(shared_data));
	search_data.nnue.force_net(net);
	search_data.reset_for_new_game();
	search_data
}

fn worker(id: usize, nodes: u64, games: u64, path: String, pairing: Pairing) -> std::io::Result<u64> {
	let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
	let mut rng = Rng::new(seed ^ (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
	let mut out = BufWriter::new(File::create(&path)?);
	let mut positions = 0;

	// One set of search state per thread, cleared between games. A new SharedData/SearchData every game
	// meant allocating and freeing ~33 MB (two TTs, the history and PV tables) per game, and on 100+ thread
	// machines the kernel's bookkeeping for that serialised the threads: cores mostly idle, each thread 8-10x
	// slower than on a 20-thread desktop.
	// Self-play: one searcher on the global net. Big vs small: [0] the big net, [1] the small one.
	let mut searchers: Vec<SearchData> = match pairing {
		Pairing::SelfPlay => vec![searcher(None)],
		Pairing::BigVsSmall => vec![searcher(Some(NetSize::Big)), searcher(Some(NetSize::Small))],
	};

	for n in 1..=games {
		// Fresh history/TT every game, like chessEngine's globalTT.clear()
		if n > 1 {
			for search_data in &mut searchers {
				search_data.reset_for_new_game();
				Arc::get_mut(&mut search_data.shared_data)
					.expect("a datagen worker's SharedData isn't shared")
					.tt
					.clear();
			}
		}

		let (board, viri) = loop {
			if let Some(opening) = random_opening(&mut rng, &mut searchers[0]) {
				break opening;
			}
		};
		// Big vs small: the big net takes white in odd games, black in even ones
		let big_color = if n % 2 == 1 { Color::White } else { Color::Black };
		let game = play_game(board, &viri, |board| {
			let i = if searchers.len() == 1 || board.side_to_move() == big_color { 0 } else { 1 };
			search(&mut searchers[i], board, nodes)
		});
		positions += game.len() as u64;
		game.serialise_into(&mut out)?;

		if n % 100 == 0 {
			out.flush()?;
			println!("info string datagen thread {id}: {n}/{games} games, {positions} positions");
		}
	}

	out.flush()?;
	Ok(positions)
}

/// `datagen <threads> <nodes> <games per thread> <output prefix>`
pub fn run(args: &[&str]) {
	let threads: usize = args.first().and_then(|v| v.parse().ok()).unwrap_or(1);
	let nodes: u64 = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(5000);
	let pairing = match args.get(4) {
		None => Pairing::DEFAULT,
		Some(m) => match Pairing::parse(m) {
			Some(p) => p,
			None => {
				println!("info string datagen: mode is self or adversarial, not {m}");
				return;
			}
		},
	};
	let games: u64 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(100);
	let prefix = args.get(3).copied().unwrap_or("datagen").to_string();

	let who = match pairing {
		Pairing::SelfPlay => "self-play",
		Pairing::BigVsSmall => "big (1024) vs small (512) net",
	};
	println!("info string datagen: {threads} threads x {games} games at {nodes} nodes, {who} -> {prefix}_<n>.vf");

	let handles: Vec<_> = (0..threads)
		.map(|id| {
			let path = format!("{prefix}_{id}.vf");
			std::thread::spawn(move || worker(id, nodes, games, path, pairing))
		})
		.collect();

	let mut total = 0;
	for handle in handles {
		match handle.join() {
			Ok(Ok(positions)) => total += positions,
			Ok(Err(e)) => println!("info string datagen thread failed: {e}"),
			Err(_) => println!("info string datagen thread panicked"),
		}
	}
	println!("info string datagen done: {total} positions");
}
