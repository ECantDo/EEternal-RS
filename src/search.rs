use crate::board::generate_moves::AllMoves;
use crate::history::noisy_key;
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

pub trait DisplayOutput {
    const DISPLAY: bool;
}

pub struct PrintOutput;
impl DisplayOutput for PrintOutput {
    const DISPLAY: bool = true;
}

pub struct NoOutput;
impl DisplayOutput for NoOutput {
    const DISPLAY: bool = false;
}

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

pub fn start_search<Display: DisplayOutput>(search_data: &mut SearchData) -> Move {
    let max_depth = match search_data.time_manager.limits() {
        Limits::Depth(depth) => depth as usize,
        _ => MAX_PLY - 1,
    };

    search_data.shared_data.tt.new_search();
    search_data.nnue.reset(&search_data.board);

    search_data.root_move.mv = Move::NONE;

    search_data.pv_table.clear(0);

    let mut prev_score: i32 = 0;

    for root_depth in 1..=max_depth {
        search_data.root_move.previous_score = search_data.root_move.score;

        // ============ Aspiration Windows ============
        // Search a narrow window around the last score, widening on fail high/low.
        // Shallow depths are too noisy, so they use the full window.
        let mut delta = 25;
        let (mut alpha, mut beta) = if root_depth >= 4 {
            (
                (prev_score - delta).max(-Score::INF),
                (prev_score + delta).min(Score::INF),
            )
        } else {
            (-Score::INF, Score::INF)
        };

        let score = loop {
            let score = search::<Root>(search_data, alpha, beta, root_depth as i32, 0, false);

            if score.abs() >= Score::NONE || search_data.shared_data.stop.load(Ordering::Relaxed) {
                break score;
            }

            if score <= alpha {
                beta = (alpha + beta) / 2;
                alpha = (score - delta).max(-Score::INF);
            } else if score >= beta {
                beta = (score + delta).min(Score::INF);
            } else {
                break score;
            }

            delta += delta / 2;
            if delta > 1000 {
                alpha = -Score::INF;
                beta = Score::INF;
            }
        };

        // A NONE-ish score means we bailed mid-search (time/stop). root_move
        // already reflects the best move found before the cutoff, so just
        // report it and go home.
        if score.abs() >= Score::NONE || search_data.shared_data.stop.load(Ordering::Relaxed) {
            if Display::DISPLAY {
                println!("{}", search_data.to_uci_info());
            }
            return search_data.root_move.mv;
        }

        prev_score = score;
        search_data.completed_depth = root_depth;
        if Display::DISPLAY {
            println!("{}", search_data.to_uci_info());
        }
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
        if root_depth >= 6 && search_data.root_move.score.abs() >= Score::MATE_IN_MAX {
            return search_data.root_move.mv;
        }
    }

    search_data.root_move.mv
}

fn search<Node: NodeType>(
	search_data: &mut SearchData,
	mut alpha: i32,
	beta: i32,
	mut depth: i32,
	ply: i32,
	cut_node: bool,
) -> i32 {
    debug_assert!(ply as usize <= MAX_PLY);
    debug_assert!(-Score::INF <= alpha && alpha < beta && beta <= Score::INF);

    search_data.shared_data.nodes.increment();

    if !Node::ROOT && Node::PV {
        search_data.pv_table.clear(ply as usize);
    }

    // Once the hard limit trips, set stop so every other node bails out immediately too
    if search_data.time_manager.check_time(search_data) {
        search_data.shared_data.stop.store(true, Ordering::Relaxed);
    }
    if search_data.shared_data.stop.load(Ordering::Relaxed) {
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
    let in_check = search_data.board.in_check();

    let static_eval = if in_check {
        -Score::INF
    } else {
        search_data.evaluate()
    };

    // IIR
    if depth >= 4 && tt_probe.best_move == Move::NONE && (Node::PV || cut_node) {
        depth -= 1;
    }

    let depth2 = depth * depth;

    // =============================================================================================
    // Razoring
    #[cfg(feature = "razoring")]
    if !Node::PV && !in_check && alpha < 2048 && !tt_probe.best_move.is_quiet() {
        if static_eval < alpha - 250 - 250 * depth2 {
            return qsearch::<NonPV>(search_data, alpha, beta, ply);
        }
    }

    // =============================================================================================
    // Reverse futility pruning
    const RFP_MARGIN: i32 = 90;

    if !Node::PV
        && !in_check
        && static_eval - RFP_MARGIN * depth >= beta
        && depth <= 8
        && beta.abs() < Score::MATE_IN_MAX
    {
        return lerp(static_eval, beta, 0.5);
    }

    // =============================================================================================
    // Null move
    if cut_node
        && !in_check
        && depth >= 3
        && (static_eval >= beta)
        && beta.abs() < Score::MATE_IN_MAX
        && search_data.board.has_non_pawn_material()
    {
        let r = 3 + depth / 3;
        search_data.board.make_null_move();
        let score = -search::<NonPV>(search_data, -beta, -beta + 1, depth - r, ply + 1, false);
        search_data.board.undo_null_move();

        if score.abs() >= Score::NONE {
            // search abort
            return score;
        }

        if score >= beta {
            // giving them a full turn doesn't matter -- can return
            return if score >= Score::MATE_IN_MAX {
                beta
            } else {
                score
            };
        }
        // Otherwise we need a full search
    }

    // ============ Generate Moves ============
    let mut moves = search_data.board.generate_all_legal_moves::<AllMoves>();

    if moves.is_empty() {
        // Draw/Mate check
        if in_check {
            return Score::mated_in(ply);
        }
        return 0;
    }

    let mut ordered_moves = OrderedMoves::new(&mut moves);
    ordered_moves.score_moves(search_data, tt_probe.best_move, ply);

    // ============ Search ============
    let mut moves_searched: usize = 0;
    let mut best_score = -Score::INF;
    let mut best_move = Move::NONE;
    let alpha_orig = alpha;

    /*
     * Len of 32 since the average moves is ~35, and some moves in there are going to be non-quiet.
     *  And even less since the first 5 moves should almost always contain the best move (according
     *  to my engine). 32 seems like a good balance
     */
    let mut quiet_moves_tried: [Move; 32] = [Move::NONE; 32];
    let mut quiet_moves_tried_len: usize = 0;

    /*
     * Len of 16 since the max number of captures in a position is 15; 8 pawns, 2 rooks, 2 knights,
     * 2 bishops, 1 queen; +1 king (kinda, but the king cant really be captured, but close enough)
     * = 16 possible captures. Having a length of 8 would likely be fine too. (gonna start with 16).
     */
    let mut noisy_moves_tried: [Move; 16] = [Move::NONE; 16];
    let mut noisy_moves_tried_len: usize = 0;

    for move_entry in ordered_moves {
        let mv = move_entry.mv();
        let is_quiet = mv.is_quiet();

        // SEE Pruning
        // if !Node::ROOT && best_score > -Score::MATE_IN_MAX // ah
		// 	&& depth <= 8
        // // Only near the leaves where these types of moves would be bad to look at
        // {
        //     let threshold = if is_quiet { -64 * depth } else { -20 * depth2 };
        //     if search_data.board.see(mv) < threshold {
        //         continue;
        //     }
        // }

        // =========================================================================================
        // Late move pruning
        // Moves searched later are likely bad
        if !Node::ROOT
            && !in_check
            && is_quiet
            && best_score > -Score::MATE_IN_MAX
            && !mv.is_promotion()
            && (moves_searched as i32) >= 3 + depth2
        {
            continue;
        }

        search_data.make_move(mv);

        #[cfg(feature = "prefetch")]
        search_data
            .shared_data
            .tt
            .prefetch(search_data.board.hash());

        let mut check_extensions: i32 = 0;
        let mut lmr_reductions: i32 = 0;
        if search_data.board.in_check() {
            check_extensions = 1; // Check extensions
        }
        if depth >= 2 && moves_searched >= 2 {
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
            // Null-move should always be false for the PV line
            score = -search::<PV>(search_data, -beta, -alpha, new_depth, ply + 1, false);
        } else {
            // Null-window search; just show move isn't better
            score = -search::<NonPV>(
                search_data,
                -alpha - 1,
                -alpha,
                new_depth,
                ply + 1,
                lmr_reductions > 0 || !cut_node,
            );

            // LMR re-search at full depth is the reduced beat alpha
            // An aborted search (|score| >= NONE) must never trigger a re-search
            let aborted = score.abs() >= Score::NONE;

            if !aborted && lmr_reductions > 0 && score > alpha {
                // If lmr > 0; we cannot be in check, so don't add in the check extension
                score = -search::<NonPV>(
                    search_data,
                    -alpha - 1,
                    -alpha,
                    depth + check_extensions - 1,
                    ply + 1,
                    !cut_node,
                );
            }

            // PVS re-search at full window if this move raised alpha, and we're on a PV node
            if Node::PV && score.abs() < Score::NONE && score > alpha {
                score = -search::<PV>(
                    search_data,
                    -beta,
                    -alpha,
                    depth - 1 + check_extensions,
                    ply + 1,
                    false,
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
                // Only trust root moves that beat alpha: a fail-low score is just an upper bound.
                if score > alpha {
                    search_data.root_move.mv = mv;
                    search_data.root_move.score = score;
                    search_data
                        .root_move
                        .pv
                        .commit_from_pv(&search_data.pv_table, 1);
                }
            } else if Node::PV && score > alpha {
                // Below alpha the child may have been a NonPV null-window search, so row ply+1 is stale.
                search_data.pv_table.update(ply as usize, mv);
            }
            // Only need to check if score is better than alpha, when the best score changes.
            if score > alpha {
                alpha = score;
                // Only need to check if alpha is better than beta when alpha changes
                if alpha >= beta {
                    let bonus = history_bonus(depth);
                    let stm = search_data.board.side_to_move();
                    let threats = search_data.board.attacks_by_color(!stm);

                    if is_quiet {
                        // Quiet Move History Heuristic
                        // threats = squares attacked by the OPPONENT of `stm` (the mover).
                        // Must match score_quiet()'s convention exactly, or update()/get() write
                        // and read different table slots for the same real position.

                        search_data.quiet_history.update(threats, stm, mv, bonus);
                        for &prev in &quiet_moves_tried[..quiet_moves_tried_len] {
                            search_data.quiet_history.update(threats, stm, prev, -bonus);
                        }

                        // Killers
                        search_data.killers.append(ply, mv);
                    } else {
                        // if !is_quiet {
                        let (piece, to_sq) = noisy_key(&search_data.board, mv, stm);

                        search_data.noisy_history.update(
                            threats,
                            search_data.board.get_piece_on_square(mv.from()),
                            to_sq,
                            piece,
                            bonus,
                        );
                        for &prev in &noisy_moves_tried[..noisy_moves_tried_len] {
                            let (piece, to_sq) = noisy_key(&search_data.board, prev, stm);

                            search_data.noisy_history.update(
                                threats,
                                search_data.board.get_piece_on_square(prev.from()),
                                to_sq,
                                piece,
                                -bonus,
                            );
                        }
                    }

                    break;
                }
            }
        }
        // Update tested quiets
        if is_quiet && quiet_moves_tried_len < 32 {
            // If we end up > 32, Woah, complex position.
            quiet_moves_tried[quiet_moves_tried_len] = mv;
            quiet_moves_tried_len += 1;
        }
        if !is_quiet && noisy_moves_tried_len < 16 {
            noisy_moves_tried[noisy_moves_tried_len] = mv;
            noisy_moves_tried_len += 1;
        }

        // Update moves searched
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

/// like basically everything else, this needs tuning
fn history_bonus(depth: i32) -> i32 {
    (depth * depth * 4).min(1600)
}

fn lerp(a: i32, b: i32, t: f32) -> i32 {
    t.mul_add((b - a) as f32, a as f32) as i32
}
