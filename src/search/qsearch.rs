use std::sync::atomic::Ordering;
use crate::search::search_types::SearchData;
use crate::search::{NodeType, NonPV};
use crate::types::moves::Move;
use crate::types::score::Score;
use crate::types::MAX_PLY;

pub fn qsearch<NODE: NodeType>(
    search_data: &mut SearchData,
    mut alpha: i32,
    beta: i32,
    ply: i32,
) -> i32 {
    debug_assert!(!NODE::ROOT);
    debug_assert!(ply as usize <= MAX_PLY);
    debug_assert!(-Score::INF <= alpha && alpha < beta && beta <= Score::INF);
    debug_assert!(NODE::PV || alpha == beta - 1);

    search_data.shared_data.nodes.increment();

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

    let stand_pat: i32;
    if in_check {
        stand_pat = Score::mated_in(ply);
    } else {
        stand_pat = search_data.board.evaluate();
        if stand_pat >= beta {
            return stand_pat;
        }

        if stand_pat > alpha {
            alpha = stand_pat;
        }
    }

    if ply as usize >= 10 { // Should be max ply ; 10 since run-away recursion is a problem
        return stand_pat;
    }

    let move_list = search_data.board.generate_all_legal_moves(!in_check);

    if move_list.is_empty() {
        if in_check {
            return Score::mated_in(ply as i32);
        }
        return stand_pat;
    }

    let mut best_score: i32 = stand_pat;

    for mv in &move_list {
        search_data.board.make_move(mv);
        let score = -qsearch::<NonPV>(search_data, -beta, -alpha, ply + 1);
        search_data.board.undo_move(mv);

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
            break
        }
    }

    best_score
}
