use crate::board::generate_moves::{CapturesOnly, QuietsOnly};
use crate::history::noisy_key;
use crate::types::move_list::MoveEntry;
use crate::{
	search::search_types::SearchData,
	types::{move_list::MoveList, moves::Move},
};

pub struct OrderedMoves<'a> {
	move_list: &'a mut MoveList,
	idx: usize,
}

pub const TT_VALUE: i32 = 20_000_000;
pub const CAPTURE_VALUE: i32 = 1_000_000;
pub const NEGATIVE_SEE_CAPTURE_VALUE: i32 = 80_000;
pub const KILLER_VALUE: i32 = 900_000;

impl<'a> OrderedMoves<'a> {
	pub fn new(move_list: &'a mut MoveList) -> Self {
		Self { move_list, idx: 0 }
	}

	pub fn score_moves(&mut self, search_data: &SearchData, tt_move: Move, ply: i32) {
		let killers = search_data.killers.get(ply);

		for idx in 0..self.move_list.len() {
			let mv = self.move_list.get(idx).mv();
			let mut score: i32 = if mv == tt_move {
				debug_assert!(mv != Move::NONE);
				TT_VALUE
			} else if mv.is_capture() {
				let see = search_data.board.see(mv);
				if see < 0 {
					NEGATIVE_SEE_CAPTURE_VALUE + see
				} else {
					CAPTURE_VALUE + see
				}
			} else if mv == killers[0] {
				KILLER_VALUE + 10
			} else if mv == killers[1] {
				KILLER_VALUE
			} else {
				score_quiet(search_data, mv, ply)
			};

			if mv.is_capture() {
				score += score_noisy(search_data, mv);
			}
			self.move_list.set_score(idx, score);
		}
	}

	pub fn select_next_best(&mut self) -> MoveEntry {
		debug_assert!(self.idx < self.move_list.len());
		let mut best_idx = self.idx;
		let mut best_move = self.move_list.get(best_idx);

		let mut i = self.idx + 1;
		while i < self.move_list.len() {
			if self.move_list.get(i).score() > best_move.score() {
				best_idx = i;
				best_move = self.move_list.get(i);
			}
			i += 1;
		}

		self.move_list.swap(self.idx, best_idx);
		self.idx += 1;
		best_move
	}
}

impl<'a> Iterator for OrderedMoves<'a> {
	type Item = MoveEntry;

	fn next(&mut self) -> Option<Self::Item> {
		if self.idx >= self.move_list.len() {
			return None;
		}
		Some(self.select_next_best())
	}
}

fn score_quiet(search_data: &SearchData, mv: Move, _ply: i32) -> i32 {
	// Get their threats ... haven't made the move yet, so look to where they are attacking
	let my_side = search_data.board.side_to_move();
	let threats = search_data.board.attacks_by_color(!my_side);
	// let occ = search_data.board.occupancies();

	// Pawn, Knight, Bishop, Rook, Queen, King
	// TODO: This
	// let escape_attack_score: [i32; PieceType::NUM] = [0, 8000, 8000, 15000, 20000, 0];
	// let pt: PieceType = search_data.board.get_piece_on_square(mv.from()).piece_type();
	// score += escape[pt] * <is threatened?>

	search_data.quiet_history.get(threats, my_side, mv)
}

fn score_noisy(search_data: &SearchData, mv: Move) -> i32 {
	let my_side = search_data.board.side_to_move();
	let threats = search_data.board.attacks_by_color(!my_side);

	let (piece, to_sq) = noisy_key(&search_data.board, mv, search_data.board.side_to_move());

	search_data.noisy_history.get(
		threats,
		search_data.board.get_piece_on_square(mv.from()),
		to_sq,
		piece,
	)
}

// =================================================================================================
// Staged move picker
// =================================================================================================
/// Hands out moves one stage at a time, so a cutoff early on (TT move, a good capture, a killer)
/// never pays for generating and scoring the quiet moves.
/// The order matches the full-list scoring above:
/// TT move -> good captures -> killers -> bad captures -> quiets
///
/// Everything lives in one MoveList: `[good captures | bad captures | quiets]`
#[derive(Copy, Clone, Eq, PartialEq)]
enum Stage {
	TTMove,
	GenerateNoisy,
	GoodNoisy,
	Killer1,
	Killer2,
	BadNoisy,
	GenerateQuiet,
	Quiet,
	Done,
}

pub struct MovePicker {
	stage: Stage,
	tt_move: Move,
	killers: [Move; 2],
	ply: i32,
	moves: MoveList,
	/// Next entry to hand out in the current range
	idx: usize,
	/// End of the good captures (start of the bad ones)
	good_end: usize,
	/// End of all captures (start of the quiets)
	noisy_end: usize,
	/// Set by LMP: only quiet promotions are still wanted from the quiet stage
	skip_quiets: bool,
}

impl MovePicker {
	pub fn new(tt_move: Move, killers: [Move; 2], ply: i32) -> Self {
		Self {
			stage: Stage::TTMove,
			tt_move,
			killers,
			ply,
			moves: MoveList::new(),
			idx: 0,
			good_end: 0,
			noisy_end: 0,
			skip_quiets: false,
		}
	}

	/// LMP has started pruning quiets, and it keeps pruning every later one (its condition can't
	/// become false again), so there's no point sorting them. Promotions are exempt from LMP, so
	/// those still come out.
	pub fn skip_quiets(&mut self) {
		self.skip_quiets = true;
	}

	pub fn next(&mut self, search_data: &SearchData) -> Option<Move> {
		let board = &search_data.board;

		loop {
			match self.stage {
				Stage::TTMove => {
					self.stage = Stage::GenerateNoisy;
					// The TT move comes from another position with the same hash (or a collision),
					// so it has to be checked before it can be played
					if board.is_pseudo_legal(self.tt_move) && board.is_legal(self.tt_move) {
						return Some(self.tt_move);
					}
				}
				Stage::GenerateNoisy => {
					self.stage = Stage::GoodNoisy;
					board.append_legal_moves::<CapturesOnly>(&mut self.moves);
					self.remove_from(0, self.tt_move);

					// Score by SEE first, and put the SEE >= 0 captures at the front
					let mut good_end = 0;
					for i in 0..self.moves.len() {
						let see = board.see(self.moves.get(i).mv());
						self.moves.set_score(i, see);
						if see >= 0 {
							self.moves.swap(i, good_end);
							good_end += 1;
						}
					}
					self.good_end = good_end;
					self.noisy_end = self.moves.len();

					// Then add the history, same as the full-list ordering (SEE + noisy history)
					for i in 0..self.noisy_end {
						let entry = self.moves.get(i);
						let score = entry.score() + score_noisy(search_data, entry.mv());
						self.moves.set_score(i, score);
					}
				}
				Stage::GoodNoisy => {
					if let Some(mv) = self.select_next(self.good_end) {
						return Some(mv);
					}
					self.stage = Stage::Killer1;
				}
				Stage::Killer1 => {
					self.stage = Stage::Killer2;
					if let Some(mv) = self.valid_killer(0, board) {
						return Some(mv);
					}
				}
				Stage::Killer2 => {
					self.stage = Stage::BadNoisy;
					if let Some(mv) = self.valid_killer(1, board) {
						return Some(mv);
					}
				}
				Stage::BadNoisy => {
					// idx is at good_end here: the good captures have all been handed out
					if let Some(mv) = self.select_next(self.noisy_end) {
						return Some(mv);
					}
					self.stage = Stage::GenerateQuiet;
				}
				Stage::GenerateQuiet => {
					self.stage = Stage::Quiet;
					board.append_legal_moves::<QuietsOnly>(&mut self.moves);

					// Already handed out by an earlier stage
					self.remove_from(self.noisy_end, self.tt_move);
					self.remove_from(self.noisy_end, self.killers[0]);
					self.remove_from(self.noisy_end, self.killers[1]);

					for i in self.noisy_end..self.moves.len() {
						let mv = self.moves.get(i).mv();
						self.moves.set_score(i, score_quiet(search_data, mv, self.ply));
					}
				}
				Stage::Quiet => {
					let next = if self.skip_quiets {
						self.select_next_promotion()
					} else {
						self.select_next(self.moves.len())
					};
					if let Some(mv) = next {
						return Some(mv);
					}
					self.stage = Stage::Done;
				}
				Stage::Done => return None,
			}
		}
	}

	/// Killers are quiet moves from sibling nodes, so they might not be legal here. A killer that
	/// became a capture fails `is_pseudo_legal` (its flag no longer matches the board).
	fn valid_killer(&self, idx: usize, board: &crate::board::Board) -> Option<Move> {
		let mv = self.killers[idx];
		// Captures are handed out in the noisy stages, so a capture here would be a duplicate
		if mv.is_capture() || mv == self.tt_move || (idx == 1 && mv == self.killers[0]) {
			return None;
		}
		(board.is_pseudo_legal(mv) && board.is_legal(mv)).then_some(mv)
	}

	/// Swap-removes `mv` from the entries at `start..`. Only used right after generating, before
	/// that range's order matters.
	fn remove_from(&mut self, start: usize, mv: Move) {
		if mv == Move::NONE {
			return;
		}
		for i in start..self.moves.len() {
			if self.moves.get(i).mv() == mv {
				self.moves.remove(i);
				return;
			}
		}
	}

	/// Like `select_next` over the quiets, but only considering promotions. The other remaining
	/// quiets are dropped.
	fn select_next_promotion(&mut self) -> Option<Move> {
		let mut best_idx = None;
		for i in self.idx..self.moves.len() {
			let entry = self.moves.get(i);
			if entry.mv().is_promotion()
				&& best_idx.is_none_or(|b| entry.score() > self.moves.get(b).score())
			{
				best_idx = Some(i);
			}
		}

		let best_idx = best_idx?;
		self.moves.swap(self.idx, best_idx);
		let mv = self.moves.get(self.idx).mv();
		self.idx += 1;
		Some(mv)
	}

	/// Selection sort step within `idx..end`: move the best remaining entry to `idx` and return it.
	/// Same tie-breaking as `OrderedMoves::select_next_best` (first highest score wins).
	fn select_next(&mut self, end: usize) -> Option<Move> {
		if self.idx >= end {
			return None;
		}

		let mut best_idx = self.idx;
		for i in self.idx + 1..end {
			if self.moves.get(i).score() > self.moves.get(best_idx).score() {
				best_idx = i;
			}
		}

		self.moves.swap(self.idx, best_idx);
		let mv = self.moves.get(self.idx).mv();
		self.idx += 1;
		Some(mv)
	}
}

#[cfg(test)]
mod tests {
	use super::MovePicker;
	use crate::board::Board;
	use crate::board::generate_moves::AllMoves;
	use crate::search::search_types::{SearchData, SharedData};
	use crate::types::moves::Move;
	use std::sync::Arc;

	const FENS: &[&str] = &[
		"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
		"r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
		"r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
		"rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
		"8/8/8/K2pP2q/8/8/8/7k w - d6 0 1",
		"1r2k3/2P5/8/8/8/8/5p2/2K3R1 b - - 0 1",
	];

	/// The picker must hand out every legal move exactly once, whatever TT move and killers it
	/// is given (legal, illegal, duplicated, captures, NONE).
	#[test]
	fn picker_yields_each_legal_move_once() {
		let mut search_data = SearchData::new(Arc::new(SharedData::new()));
		let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
		let mut next_rand = move || {
			seed ^= seed << 13;
			seed ^= seed >> 7;
			seed ^= seed << 17;
			seed
		};

		for fen in FENS {
			let start = Board::parse_fen_string(fen).unwrap();
			for _ in 0..4 {
				let mut board = start.clone();
				for _ in 0..40 {
					let legal = board.generate_all_legal_moves::<AllMoves>();
					if legal.is_empty() {
						break;
					}
					let legal: Vec<Move> = (&legal).into_iter().map(|e| e.mv()).collect();
					let pick = |r: u64| legal[(r % legal.len() as u64) as usize];

					// A mix of legal moves, garbage and NONE for the TT move and killers
					let candidates = [
						Move::NONE,
						pick(next_rand()),
						pick(next_rand()),
						Move::from_raw(next_rand() as u16),
					];
					let tt = candidates[(next_rand() % 4) as usize];
					let killers = [
						candidates[(next_rand() % 4) as usize],
						candidates[(next_rand() % 4) as usize],
					];

					search_data.set_board(&board);
					let mut picker = MovePicker::new(tt, killers, 0);
					let mut seen = Vec::new();
					while let Some(mv) = picker.next(&search_data) {
						assert!(legal.contains(&mv), "illegal {} from picker, {board}", mv.to_uci());
						assert!(!seen.contains(&mv), "duplicate {} from picker, {board}", mv.to_uci());
						seen.push(mv);
					}
					assert_eq!(seen.len(), legal.len(), "picker missed moves, {board}");

					board.make_move(pick(next_rand()));
				}
			}
		}
	}
}
