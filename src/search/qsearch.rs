use crate::board::generate_moves::{AllMoves, CapturesOnly};
use crate::types::move_list::MoveList;
use crate::types::piece::PieceType;
use crate::{
	search::{
		move_ordering::{CAPTURE_VALUE, OrderedMoves},
		search_types::SearchData,
		{NodeType, NonPV},
	},
	types::{MAX_PLY, moves::Move, score::Score},
};
use crate::params::{CORRHIST_DIV, QS_DELTA_MARGIN};
use std::sync::atomic::Ordering;

pub fn qsearch<NODE: NodeType>(
	search_data: &mut SearchData,
	mut alpha: i32,
	beta: i32,
	ply: i32,
) -> i32 {
	debug_assert!(!NODE::ROOT);
	debug_assert!(ply as usize <= MAX_PLY);
	debug_assert!(-Score::INF <= alpha && alpha < beta && beta <= Score::INF);
	// debug_assert!(NODE::PV || alpha == beta - 1);

	search_data.shared_data.nodes.increment();
	search_data.sel_depth = search_data.sel_depth.max(ply as usize + 1);

	if alpha >= beta {
		return alpha;
	}

	if search_data.board.is_draw() {
		return 0;
	}

	if search_data.time_manager.check_time(search_data)
		|| search_data.shared_data.stop.load(Ordering::Relaxed)
	{
		return Score::NONE;
	}

	let in_check = search_data.board.in_check();
	let stm = search_data.board.side_to_move();

	let stand_pat: i32;
	if in_check {
		stand_pat = Score::mated_in(ply);
	} else {
		let raw_eval = search_data.evaluate();
		// /32 failed SPRT (-33 with the fail-low fix), /64 passed
		let correction: i32 = search_data
			.pawn_correction_history
			.get(stm, search_data.board.pawn_key())
			/ CORRHIST_DIV;
		stand_pat = (raw_eval + correction).clamp(-Score::MATE_IN_MAX + 1, Score::MATE_IN_MAX - 1);

		if stand_pat >= beta {
			return stand_pat;
		}
		if stand_pat > alpha {
			alpha = stand_pat;
		}
	}

	if ply as usize >= MAX_PLY {
		// Will return a mate score if in check
		return stand_pat;
	}

	let mut move_list: MoveList = if in_check {
		search_data.board.generate_all_legal_moves::<AllMoves>()
	} else {
		search_data.board.generate_all_legal_moves::<CapturesOnly>()
	};

	if move_list.is_empty() {
		// Draw/Mate check
		if in_check {
			return Score::mated_in(ply);
		}
		return stand_pat;
	}

	let mut best_score: i32 = stand_pat;
	let mut ordered_moves = OrderedMoves::new(&mut move_list);
	ordered_moves.score_moves(search_data, Move::NONE, ply);

	for move_entry in ordered_moves {
		let mv = move_entry.mv();

		// Don't prune when in check ; must search all positions
		if !in_check {
			let see_score = move_entry.score() - CAPTURE_VALUE;
			// SEE pruning: skip captures that lose material outright.
			if see_score < 0 {
				continue;
			}

			// Delta pruning: even winning this capture can't reach alpha.
			let captured_type: PieceType = if mv.is_en_passant() {
				PieceType::Pawn
			} else {
				search_data.board.get_piece_on_square(mv.to()).piece_type()
			};

			let captured_value: i32 = captured_type.value();
			if stand_pat + captured_value + QS_DELTA_MARGIN < alpha {
				continue;
			}
		}

		search_data.make_move(mv);
		let score = -qsearch::<NonPV>(search_data, -beta, -alpha, ply + 1);
		search_data.undo_move(mv);

		if score.abs() >= Score::NONE {
			return score;
		}
		if score > best_score {
			best_score = score;
		}
		if score > alpha {
			alpha = score;
		}
		if alpha >= beta {
			break;
		}
	}

	best_score
}
