use crate::search::qsearch::qsearch;
use crate::{
    search::search_types::SearchData,
    time_manager::Limits,
    types::{moves::Move, score::Score, tt::Bound, MAX_PLY},
};
use std::sync::atomic::Ordering;
use crate::search::move_ordering::OrderedMoves;

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
    let mut moves = search_data.board.generate_all_legal_moves(false);
    debug_assert!(
        !moves.is_empty(),
        "start_search called on a position with no legal moves"
    );

    let max_depth = match search_data.time_manager.limits() {
        Limits::Depth(depth) => depth as usize,
        _ => MAX_PLY - 1,
    };

    search_data.shared_data.tt.new_search();

    search_data.root_move.mv = moves.get(0).mv();

    for root_depth in 1..=max_depth {
        let mut alpha = -Score::INF;
        let beta = Score::INF;
        let mut best_score = -Score::INF;

        let mut idx = 0;
        for move_entry in &moves {
            let mv = move_entry.mv();
            search_data.board.make_move(mv);
            let score = -search::<Root>(search_data, -beta, -alpha, (root_depth - 1) as i32, 1);
            search_data.board.undo_move(mv);

            if score.abs() >= Score::NONE {
                println!("{}", search_data.to_uci_info());
                return search_data.root_move.mv;
            }

            if score > best_score {
                search_data.root_move.mv = mv;
                search_data.root_move.score = score;
                best_score = score;
                // place best move first
                // this is move ordering, but kinda bad move ordering...
                moves.place_first(idx); // This is fine, I swear
                // moves.swap(0, idx);
            }
            if score > alpha {
                alpha = score;
            }

            // Hard Time limit
            if search_data.time_manager.check_time(search_data)
                || search_data.shared_data.stop.load(Ordering::Relaxed)
            {
                println!("{}", search_data.to_uci_info());
                return search_data.root_move.mv; // The best move will either be the first one (still thinks is best), or a new one that is better
            }
            idx += 1
        }

        // Store the root result too — future iterations (and any tool that
        // probes the TT at the startpos) get the benefit of this depth.
        search_data.shared_data.tt.store(
            search_data.board.hash(),
            search_data.root_move.mv,
            root_depth as i32,
            best_score,
            Bound::Exact,
            0,
        );

        search_data.completed_depth = root_depth;
        println!("{}", search_data.to_uci_info());

        if search_data
            .time_manager
            .soft_limit_exceeded(&*search_data.shared_data)
            || search_data.shared_data.stop.load(Ordering::Relaxed)
        {
            break;
        }
    }

    return search_data.root_move.mv;
}

fn search<Node: NodeType>(
    search_data: &mut SearchData,
    mut alpha: i32,
    mut beta: i32,
    depth: i32,
    ply: i32,
) -> i32 {
    debug_assert!(ply as usize <= MAX_PLY);
    debug_assert!(-Score::INF <= alpha && alpha < beta && beta <= Score::INF);

    search_data.shared_data.nodes.increment();

    if search_data.time_manager.check_time(search_data)
        || search_data.shared_data.stop.load(Ordering::Relaxed)
    {
        return Score::NONE;
    }

    if search_data.board.is_draw() {
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
    let mut best_score = -Score::INF;
    let mut best_move = tt_probe.best_move; // used for ordering later; fine as-is for now ; none by default
    let alpha_orig = alpha;

    for move_entry in ordered_moves {
        let mv = move_entry.mv();
        search_data.board.make_move(mv);
        let score = -search::<NonPV>(search_data, -beta, -alpha, depth - 1, ply + 1);
        search_data.board.undo_move(mv);
        search_data.nnue.pop();

        if score.abs() >= Score::NONE {
            return score;
        }

        debug_assert!(score.abs() <= Score::MATE);

        if score > best_score {
            best_score = score;
            best_move = mv;
        }
        if score > alpha {
            alpha = score;
        }
        if alpha >= beta {
            break;
        }
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
