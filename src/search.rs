use crate::search::move_ordering::OrderedMoves;
use crate::search::qsearch::qsearch;
use crate::search::search_types::LMR_TABLE;
use crate::{
    search::search_types::SearchData,
    time_manager::Limits,
    types::{MAX_PLY, moves::Move, score::Score, tt::Bound},
};
use std::sync::atomic::Ordering;

mod move_ordering;
pub mod qsearch;
pub mod search_types;
pub mod see;

pub trait NodeType {
    const PV: bool;
    const ROOT: bool;
}

struct Root;
impl NodeType for Root {
    const PV: bool = true;
    const ROOT: bool = true;
}

struct PV;
impl NodeType for PV {
    const PV: bool = true;
    const ROOT: bool = false;
}

struct NonPV;
impl NodeType for NonPV {
    const PV: bool = false;
    const ROOT: bool = false;
}

pub fn start_search(search_data: &mut SearchData) -> Move {
    let max_depth = match search_data.time_manager.limits() {
        Limits::Depth(depth) => depth as usize,
        _ => MAX_PLY - 1,
    };

    search_data.shared_data.tt.new_search();
    search_data.nnue.reset(&search_data.board);

    search_data.root_move.mv = Move::None;

    search_data.pv_table.clear(0);

    for root_depth in 1..=max_depth {
        let score = search::<Root>(search_data, -Score::INF, Score::INF, root_depth as i32, 0);

        // A NONE-ish score means we bailed mid-search (time/stop). root_move
        // already reflects the best move found before the cutoff, so just
        // report it and go home.
        if score.abs() >= Score::NONE || search_data.shared_data.stop.load(Ordering::Relaxed) {
            println!("{}", search_data.to_uci_info());
            return search_data.root_move.mv;
        }

        search_data.completed_depth = root_depth;
        println!("{}", search_data.to_uci_info());

        if search_data
            .time_manager
            .soft_limit_exceeded(&*search_data.shared_data)
            || search_data.shared_data.stop.load(Ordering::Relaxed)
        {
            break;
        }

        // Search to a depth of AT LEAST 6, and if the best move is mate, just return that move,
        // as it will be very hard to find better (it might not be the fastest mate ... but still).
        // I want to implement a checkmate search at some point, so IF the score is Mate, switch to
        // that to try and find a faster mate.
        // Also works for if there are no legal moves of quitting early.
        if root_depth <= 6 && search_data.root_move.score.abs() >= Score::MATE_IN_MAX {
            return search_data.root_move.mv;
        }
    }

    search_data.root_move.mv
}

fn search<Node: NodeType>(
    search_data: &mut SearchData,
    mut alpha: i32,
    beta: i32,
    depth: i32,
    ply: i32,
) -> i32 {
    debug_assert!(ply as usize <= MAX_PLY);
    debug_assert!(-Score::INF <= alpha && alpha < beta && beta <= Score::INF);

    search_data.shared_data.nodes.increment();

    if !Node::ROOT && Node::PV {
        search_data.pv_table.clear(ply as usize);
    }

    if search_data.time_manager.check_time(search_data)
        || search_data.shared_data.stop.load(Ordering::Relaxed)
    {
        return Score::NONE;
    }

    if search_data.board.is_draw() && !Node::ROOT {
        return 0;
    }

    // ============ Evaluate on depth 0 ============
    if depth <= 0 {
        return qsearch::<NonPV>(search_data, alpha, beta, ply);
    }

    // ============ TT Probe ============
    let hash = search_data.board.hash();
    let tt_probe = search_data
        .shared_data
        .tt
        .probe(hash, depth, alpha, beta, ply);

    if let Some(score) = tt_probe.score {
        // Don't cut PV nodes short on a TT hit — we need to walk this line
        // ourselves to build an accurate principal variation, not just know
        // its final score.
        if !Node::PV {
            return score;
        }
    }

    // ============ Generate Moves ============
    let mut moves = search_data.board.generate_all_legal_moves(false);
    let in_check = search_data.board.in_check();

    if moves.is_empty() {
        // Draw/Mate check
        if in_check {
            return Score::mated_in(ply);
        }
        return 0;
    }

    let mut ordered_moves = OrderedMoves::new(&mut moves);
    ordered_moves.score_moves(search_data, tt_probe.best_move);

    // ============ Search ============
    let mut moves_searched: usize = 0;
    let mut best_score = -Score::INF;
    let mut best_move = Move::None;
    let alpha_orig = alpha;

    for move_entry in ordered_moves {
        let mv = move_entry.mv();
        search_data.make_move(mv);

        let mut check_extensions: i32 = 0;
        let mut lmr_reductions: i32 = 0;
        if search_data.board.in_check() {
            // TODO: try double check extension on PV node
            check_extensions += 1; // Check extensions
        } else if depth >= 2 && moves_searched >= 2 {
            // search the first 3 moves in full, then LMR
            // Only do LMR when not in check? (might be better)
            lmr_reductions = LMR_TABLE[depth as usize][moves_searched];
        }

        // ===================== SEARCH ====================
        let new_depth = depth + check_extensions - 1 - lmr_reductions;
        let mut score: i32;
        if moves_searched == 0 && Node::PV {
            // PV is true for Root
            // Assumed best move: give it a full search; there will never be an LMR here
            // (i.e. extensions will never be less than -1)
            score = -search::<PV>(search_data, -beta, -alpha, new_depth, ply + 1);
        } else {
            // Null-window search; just show move isn't better
            score = -search::<NonPV>(search_data, -alpha - 1, -alpha, new_depth, ply + 1);

            // LMR re-search at full depth is the reduced beat alpha
            if lmr_reductions > 0 && score > alpha {
                // If lmr > 0; we cannot be in check, so don't add in the check extension
                score = -search::<NonPV>(search_data, -alpha - 1, -alpha, depth - 1, ply + 1);
            }

            // PVS re-search at full window if this move raised alpha, and we're on a PV node
            if Node::PV && score > alpha {
                score = -search::<PV>(
                    search_data,
                    -beta,
                    -alpha,
                    depth - 1 + check_extensions,
                    ply + 1,
                );
            }
        }

        search_data.undo_move(mv);

        // Basically checking if the search ended
        if score.abs() >= Score::NONE {
            return score;
        }

        debug_assert!(score.abs() <= Score::MATE);

        if score > best_score {
            best_score = score;
            best_move = mv;

            if Node::ROOT {
                search_data.root_move.mv = mv;
                search_data.root_move.score = score;
                search_data.root_move.pv.commit_from_pv(&search_data.pv_table, 1);
            } else if Node::PV {
                search_data.pv_table.update(ply as usize, mv);
            }
            // Only need to check if score is better than alpha, when the best score changes.
            if score > alpha {
                alpha = score;
                // Only need to check if alpha is better than beta when alpha changes
                if alpha >= beta {
                    break;
                }
            }
        }

        moves_searched += 1;
    }

    // ============ TT Store ============
    let bound = if best_score >= beta {
        Bound::Lower
    } else if best_score <= alpha_orig {
        Bound::Upper
    } else {
        Bound::Exact
    };
    search_data
        .shared_data
        .tt
        .store(hash, best_move, depth, best_score, bound, ply);

    best_score
}
