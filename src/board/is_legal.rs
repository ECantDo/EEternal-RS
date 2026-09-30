use crate::attacking::{
	get_bishop_attacks, get_king_attacks, get_knight_attacks, get_pawn_attacks, get_queen_attacks,
	get_rook_attacks,
};
use crate::board::Board;
use crate::rays::{between, line_through};
use crate::types::bitboard::Bitboard;
use crate::types::move_list::MoveList;
use crate::types::moves::{Move, MoveFlag};
use crate::types::piece::{Piece, PieceType};
use crate::types::square::Square;
use crate::types::{PAWN_START, UP_DIR};

impl Board {
	/// Could `mv` have been generated in this position, ignoring whether it leaves the king in
	/// check? Used for moves that don't come from the move generator (TT move, killers), so it
	/// has to reject anything, including flags that don't match the board.
	/// `is_pseudo_legal(mv) && is_legal(mv)` <=> `mv` is in the legal move list.
	pub fn is_pseudo_legal(&self, mv: Move) -> bool {
		// 6 and 7 aren't valid flags (and `flag()` can't represent them)
		let bits = mv.flag_bits();
		if bits == 6 || bits == 7 {
			return false;
		}

		let from = mv.from();
		let to = mv.to();
		if from == to {
			return false; // Also rejects Move::NONE
		}

		let stm = self.side_to_move();
		let piece = self.get_piece_on_square(from);
		if piece == Piece::None || piece.color() != stm {
			return false;
		}
		if self.get_color(stm).contains(to) {
			return false;
		}

		let pt = piece.piece_type();
		let occ = self.occupancies();
		let enemy = self.get_color(!stm);
		let flag = mv.flag();

		if mv.is_castling() {
			if pt != PieceType::King {
				return false;
			}
			let mut ml = MoveList::new();
			self.collect_castling(&mut ml);
			return (&ml).into_iter().any(|entry| entry.mv() == mv);
		}

		if pt == PieceType::Pawn {
			if flag == MoveFlag::EnPassant {
				let ep = self.en_passant();
				return ep != Square::None && to == ep && get_pawn_attacks(from, stm).contains(to);
			}

			// Moving onto the last rank must be a promotion, and a promotion must move onto it
			if mv.is_promotion() != Bitboard::BOTH_HOME_ROWS.contains(to) {
				return false;
			}

			if mv.is_capture() {
				return get_pawn_attacks(from, stm).contains(to) && enemy.contains(to);
			}

			let up = UP_DIR[stm];
			let one = from.shift(up);
			if flag == MoveFlag::DoublePush {
				return from.get_rank() == PAWN_START[stm]
					&& to == one.shift(up)
					&& !occ.contains(one)
					&& !occ.contains(to);
			}
			// Single push or quiet promotion
			return to == one && !occ.contains(to);
		}

		// Everything else only ever has Quiet or Capture flags
		match flag {
			MoveFlag::Quiet if !occ.contains(to) => {}
			MoveFlag::Capture if enemy.contains(to) => {}
			_ => return false,
		}

		let attacks = match pt {
			PieceType::Knight => get_knight_attacks(from),
			PieceType::Bishop => get_bishop_attacks(from, occ),
			PieceType::Rook => get_rook_attacks(from, occ),
			PieceType::Queen => get_queen_attacks(from, occ),
			PieceType::King => get_king_attacks(from),
			_ => return false,
		};
		attacks.contains(to)
	}

	pub fn is_legal(&self, mv: Move) -> bool {
		let stm = self.side_to_move();
		let king = self.king_square(stm);
		let from = mv.from();
		let to = mv.to();

		if self.get_piece_on_square(from).piece_type() == PieceType::King {
			if mv.is_castling() {
				return true; // path/attacked-square checks already done at generation time
			}
			return !self.board_state.threats_by[!stm].contains(to);
		}

		let checkers = self.checkers();
		if checkers.not_empty() {
			if checkers.popcount() > 1 {
				return false; // only the king escapes double check
			}
			let checker_sq = checkers.lsb();
			// For en passant, the square that actually resolves the check is
			// the captured pawn's square, not the empty landing square.
			let relevant_sq = if mv.is_en_passant() {
				to.shift(-UP_DIR[stm])
			} else {
				to
			};
			if !(checkers | between(king, checker_sq)).contains(relevant_sq) {
				return false;
			}
		}

		if mv.is_en_passant() {
			// The classic case a plain pin check can't catch: both the captured
			// and moving pawn vanish from the same rank, opening a rook/queen
			// attack on the king that wasn't there with either pawn still on it.
			let cap_sq = to.shift(-UP_DIR[stm]);
			let occ =
				self.occupancies() ^ from.to_bitboard() ^ to.to_bitboard() ^ cap_sq.to_bitboard();
			let diagonal = self.colored_pieces(!stm, PieceType::Bishop)
				| self.colored_pieces(!stm, PieceType::Queen);
			let orthogonal = self.colored_pieces(!stm, PieceType::Rook)
				| self.colored_pieces(!stm, PieceType::Queen);
			if (get_bishop_attacks(king, occ) & diagonal).not_empty()
				|| (get_rook_attacks(king, occ) & orthogonal).not_empty()
			{
				return false;
			}
			return true;
		}

		if self.pinned(stm).contains(from) {
			return line_through(king, from).contains(to);
		}

		true
	}
}

#[cfg(test)]
mod tests {
	use crate::board::Board;
	use crate::board::generate_moves::{AllMoves, CapturesOnly, QuietsOnly};
	use crate::types::moves::Move;

	const FENS: &[&str] = &[
		"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
		"r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
		"8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
		"r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
		"rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
		"r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
		// en passant, including the rank-pin case
		"8/8/8/K2pP2q/8/8/8/7k w - d6 0 1",
		"rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
		// promotions both ways, and a double check
		"1r2k3/2P5/8/8/8/8/5p2/2K3R1 b - - 0 1",
		"4k3/8/8/8/8/5n2/8/r3K3 w - - 0 1",
	];

	/// For every possible 16-bit move, `is_pseudo_legal && is_legal` must agree with the
	/// generator, and CapturesOnly + QuietsOnly must split AllMoves exactly.
	fn check_position(board: &Board) {
		let all = board.generate_all_legal_moves::<AllMoves>();
		let captures = board.generate_all_legal_moves::<CapturesOnly>();
		let quiets = board.generate_all_legal_moves::<QuietsOnly>();

		assert_eq!(all.len(), captures.len() + quiets.len(), "split size, {board}");
		for entry in &captures {
			assert!(entry.mv().is_capture(), "quiet in CapturesOnly, {board}");
		}
		for entry in &quiets {
			assert!(!entry.mv().is_capture(), "capture in QuietsOnly, {board}");
		}

		let legal: Vec<Move> = (&all).into_iter().map(|e| e.mv()).collect();
		for entry in (&captures).into_iter().chain(&quiets) {
			assert!(legal.contains(&entry.mv()), "split move not in AllMoves, {board}");
		}

		for raw in 0..=u16::MAX {
			let mv = Move::from_raw(raw);
			let accepted = board.is_pseudo_legal(mv) && board.is_legal(mv);
			assert_eq!(
				accepted,
				legal.contains(&mv),
				"{} (raw {raw:#06x}), {board}",
				mv.to_uci()
			);
		}
	}

	#[test]
	fn pseudo_legal_matches_generator() {
		let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
		let mut positions = 0;

		for fen in FENS {
			let start = Board::parse_fen_string(fen).unwrap();

			// A few random games from each position, checking every position along the way
			for _ in 0..4 {
				let mut board = start.clone();
				for _ in 0..40 {
					check_position(&board);
					positions += 1;

					let moves = board.generate_all_legal_moves::<AllMoves>();
					if moves.is_empty() {
						break;
					}
					seed ^= seed << 13;
					seed ^= seed >> 7;
					seed ^= seed << 17;
					board.make_move(moves.get((seed % moves.len() as u64) as usize).mv());
				}
			}
		}
		assert!(positions > 500);
	}
}
