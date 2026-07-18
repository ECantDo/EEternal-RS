use crate::board::Board;
use crate::search::search_types::{SearchData, SharedData};
use crate::search::start_search;
use crate::types::{piece::PieceType, square::Square};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

pub fn run_uci() {
    let stdin = io::stdin();
    let mut board = Board::startpos();

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));

        match cmd {
            "uci" => {
                println!("id name EeternalRS_V{}", env!("CARGO_PKG_VERSION"));
                println!("id author ECanDo");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "ucinewgame" => board = Board::startpos(),
            "position" => handle_position(&mut board, rest),
            "go" => handle_go(&mut board, rest),
            "quit" => break,
            "d" => println!("{board}"),
            _ => {} // ignore anything we don't handle yet (setoption, debug, etc.)
        }

        io::stdout().flush().unwrap();
    }
}

fn handle_position(board: &mut Board, rest: &str) {
    let (pos_part, moves_part) = match rest.find("moves") {
        Some(i) => (rest[..i].trim(), Some(rest[i + "moves".len()..].trim())),
        None => (rest.trim(), None),
    };

    if pos_part == "startpos" {
        *board = Board::startpos();
    } else if let Some(fen) = pos_part.strip_prefix("fen ") {
        match Board::parse_fen_string(fen.trim()) {
            Ok(b) => *board = b,
            Err(e) => eprintln!("bad fen: {e}"),
        }
    }

    if let Some(moves_str) = moves_part {
        for mv_str in moves_str.split_whitespace() {
            match parse_uci_move(board, mv_str) {
                Some(mv) => board.make_move(mv),
                None => eprintln!("illegal/unrecognized move: {mv_str}"),
            }
        }
    }
}

fn handle_go(board: &mut Board, rest: &str) {
    let parts: Vec<&str> = rest.split_whitespace().collect();

    if parts.len() >= 2 && parts[0] == "perft" {
        let start = std::time::Instant::now();
        let nodes = board.perft(parts[1].parse().unwrap_or(1));
        println!("Nodes: {} \t | {} ms", nodes, start.elapsed().as_millis());
    } else {
        let shared_data = SharedData::new();
        let mut search_data = SearchData::new(Arc::from(shared_data));
        search_data.set_board(board);

        let mv = start_search(&mut search_data, 5);
        println!("bestmove {}", mv.to_uci(board));
    }
}

fn parse_uci_move(board: &mut Board, uci: &str) -> Option<crate::types::moves::Move> {
    if uci.len() < 4 {
        return None;
    }

    let from = Square::try_from(&uci[0..2]).ok()?;
    let to = Square::try_from(&uci[2..4]).ok()?;

    let promo = uci.as_bytes().get(4).and_then(|&b| match b {
        b'q' => Some(PieceType::Queen),
        b'r' => Some(PieceType::Rook),
        b'b' => Some(PieceType::Bishop),
        b'n' => Some(PieceType::Knight),
        _ => None,
    });

    let legal = board.generate_all_legal_moves();
    legal.into_iter().find(|mv| {
        mv.from() == from
            && mv.to() == to
            && (!mv.is_promotion() || Some(mv.promotion_piece_type()) == promo)
    })
}
