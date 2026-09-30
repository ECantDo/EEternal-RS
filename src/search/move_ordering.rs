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

fn score_quiet(search_data: &SearchData, mv: Move, ply: i32) -> i32 {
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
