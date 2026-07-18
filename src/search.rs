use crate::search::search_types::SearchData;
use crate::types::moves::Move;
use crate::types::score::Score;
use crate::types::MAX_PLY;

pub mod search_types;

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

pub fn start_search(search_data: &mut SearchData, depth: i32) -> Move {
    let mut moves = search_data.board.generate_all_legal_moves();
    debug_assert!(
        !moves.is_empty(),
        "start_search called on a position with no legal moves"
    );

    search_data.root_move.mv = moves.get(0);
    let mut alpha = -Score::INF;
    let beta = Score::INF;

    for root_depth in 1..(depth + 1) {
        let mut best_score = -Score::INF;

        let mut idx = 0;
        for mv in &moves {
            search_data.board.make_move(mv);
            let score = -search::<Root>(search_data, -beta, -alpha, root_depth - 1, 1);
            search_data.board.undo_move(mv);

            if score > best_score {
                search_data.root_move.mv = mv;
                search_data.root_move.score = score;
                best_score = score;
                moves.place_first(idx);
            }
            if score > alpha {
                alpha = score;
            }
            idx += 1
        }
        println!("{}", search_data.to_uci_info(depth));
    }

    search_data.root_move.mv
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

    let in_check = search_data.board.in_check();

    // ============ Evaluate on depth 0 ============
    if depth <= 0 && !in_check {
        return search_data.board.evaluate();
    }

    // ============ Generate Moves ============
    let moves = search_data.board.generate_all_legal_moves();

    if moves.is_empty() {
        // Draw/Mate check
        if in_check {
            return Score::mated_in(ply);
        }
        return 0;
    } else if depth <= 0 {
        return search_data.board.evaluate();
    }

    // ============ Search ============
    let mut best_score = -Score::INF;

    for mv in &moves {
        search_data.board.make_move(mv);
        let score = -search::<NonPV>(search_data, -beta, -alpha, depth - 1, ply + 1);
        search_data.board.undo_move(mv);

        debug_assert!(score.abs() <= Score::MATE);

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
