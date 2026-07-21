use std::sync::atomic::Ordering;
use crate::search::search_types::SearchData;
use crate::search::{NodeType, NonPV};
use crate::types::moves::Move;
use crate::types::score::Score;
use crate::types::MAX_PLY;

const DELTA_MARGIN: i32 = 200;

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

    if ply as usize >= MAX_PLY {
        return if in_check { search_data.board.evaluate() } else { stand_pat };
    }

    let move_list = search_data.board.generate_all_legal_moves(!in_check);
    let mut best_score: i32 = stand_pat;

    if !in_check {
        // Order captures by SEE (best trades first) so cutoffs happen sooner,
        // and prune losing/hopeless captures before ever making the move.
        let mut scored: Vec<(Move, i32)> = (&move_list)
            .into_iter()
            .map(|mv| (mv, search_data.board.see(mv)))
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));

        for (mv, see_score) in scored {
            // SEE pruning: skip captures that lose material outright.
            if see_score < 0 {
                continue;
            }

            // Delta pruning: even winning this capture can't reach alpha.
            let captured_value = search_data.board.get_piece_on_square(mv.to()).value().abs();
            if stand_pat + captured_value + DELTA_MARGIN < alpha {
                continue;
            }

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
                break;
            }
        }
    } else {
        // In check: must search every evasion fully — no pruning, we need
        // to know whether we can escape at all.
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
                break;
            }
        }
    }

    best_score
}