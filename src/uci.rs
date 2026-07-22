use crate::{
    board::Board,
    search::{
        search_types::{SearchData, SharedData},
        start_search,
    },
    time_manager::{Limits, TimeManager},
    types::{
        {color::Color, moves::Move},
        {piece::PieceType, square::Square},
    },
};
use std::{
    io::{self, BufRead, Write},
    sync::{atomic::Ordering, Arc},
};
use crate::types::move_list::MoveEntry;

pub fn run_uci() {
    let stdin = io::stdin();
    let mut board = Board::startpos();
    let mut search_thread: Option<std::thread::JoinHandle<()>> = None;
    let shared_data = Arc::new(SharedData::new());

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Error! {e}");
                break;
            }
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));

        match cmd {
            "uci" => {
                println!("id name EeternalRS_V{}", env!("CARGO_PKG_VERSION"));
                println!("id author ECanDo");
                println!("option name EvalFile type string default <empty>");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "ucinewgame" => board = Board::startpos(),
            "position" => handle_position(&mut board, rest),
            "go" => {
                if let Some(handle) = search_thread.take() {
                    shared_data.stop.store(true, Ordering::Relaxed);
                    let _ = handle.join(); // wait for the previous search to actually finish
                }
                shared_data.stop.store(false, Ordering::Relaxed);
                shared_data.nodes.reset();
                let board_clone = board.clone();
                let rest = rest.to_string();
                let shared_data = Arc::clone(&shared_data);
                search_thread = Some(std::thread::spawn(move || {
                    handle_go(board_clone, &rest, shared_data);
                }));
            }
            "setoption" => handle_setoption(rest),
            "stop" => shared_data.stop.store(true, Ordering::Relaxed),
            "quit" => {
                shared_data.stop.store(true, Ordering::Relaxed);
                break;
            }
            "d" => println!("{board}"),
            _ => {
                eprintln!("Unknown option {}", line)
            } // ignore anything we don't handle yet (setoption, debug, etc.)
        }

        io::stdout().flush().unwrap();
    }

    if let Some(handle) = search_thread.take() {
        shared_data.stop.store(true, Ordering::Relaxed);
        let _ = handle.join();
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
                Some(move_entry) => board.make_move(move_entry.mv()),
                None => eprintln!("illegal/unrecognized move: {mv_str}"),
            }
        }
    }
}

fn handle_go(mut board: Board, rest: &str, shared_data: Arc<SharedData>) {
    let parts: Vec<&str> = rest.split_whitespace().collect();

    if parts.len() >= 1 && parts[0].contains("perft") {
        let start = std::time::Instant::now();
        let nodes: u64;
        let perft_depth: usize = parts.get(1).unwrap_or(&"4").parse().unwrap_or(4);
        if parts[0] == "perft" {
            nodes = board.perft(perft_depth);
        } else if parts[0] == "bulkperft" {
            nodes = board.perft_bulk(perft_depth);
        } else {
            nodes = 0;
        }
        println!("Nodes: {} \t | {} ms", nodes, start.elapsed().as_millis());
        return;
    }

    let limits = parse_limits(board.side_to_move(), &*parts);

    if matches!(limits, Limits::Infinite) {
        return; // Can do inf, no way to stop
    }

    let mut search_data = SearchData::new(Arc::from(shared_data));
    search_data.set_board(&board);
    // TODO : Figure out overhead (guessing 15ms)
    search_data.time_manager = TimeManager::new(limits, board.full_move_number(), 15);

    let mv = start_search(&mut search_data);
    println!("bestmove {}", mv.to_uci(&board));
    io::stdout().flush().unwrap();
}

fn parse_limits(color: Color, tokens: &[&str]) -> Limits {
    if let ["infinite"] = tokens {
        return Limits::Infinite;
    }

    let mut main = None;
    let mut inc = None;
    let mut moves = None;

    for chunk in tokens.chunks(2) {
        if let [name, value] = *chunk {
            let Ok(value) = value.parse::<u64>() else {
                continue;
            };

            match name {
                "depth" if value > 0 => return Limits::Depth(value as i32),
                "movetime" if value > 0 => return Limits::Time(value),
                "nodes" if value > 0 => return Limits::Nodes(value),
                // "mate" if value > 0 => return Limits::Mate(value), // Can't mate search ...
                "wtime" if Color::White == color => main = Some(value),
                "btime" if Color::Black == color => main = Some(value),
                "winc" if Color::White == color => inc = Some(value),
                "binc" if Color::Black == color => inc = Some(value),
                "movestogo" => moves = Some(value),

                _ => continue,
            }
        }
    }

    if main.is_none() && inc.is_none() {
        return Limits::Infinite;
    }

    let main = main.unwrap_or_default();
    let inc = inc.unwrap_or_default();

    match moves {
        Some(moves) => Limits::Cyclic(main, inc, moves),
        None => Limits::Fischer(main, inc),
    }
}

fn parse_uci_move(board: &mut Board, uci: &str) -> Option<MoveEntry> {
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

    let legal = board.generate_all_legal_moves(false);
    legal
        .into_iter()
        .find(|move_entry| {
            move_entry.mv().from() == from
                && move_entry.mv().to() == to
                && (!move_entry.mv().is_promotion()
                    || Some(move_entry.mv().promotion_piece_type()) == promo)
        })
        
}

fn handle_setoption(rest: &str) {
    let Some(after_name) = rest.strip_prefix("name ") else {
        return;
    };

    let Some(value_idx) = after_name.find(" value ") else {
        return;
    };

    let name = after_name[..value_idx].trim();
    let value = after_name[value_idx + " value ".len()..].trim();

    if name.eq_ignore_ascii_case("EvalFile") {
        match crate::nnue::init_from_file(value) {
            Ok(()) => println!("info string Loaded NNUE network from {value}"),
            Err(e) => eprintln!("info string {e}"),
        }
    }
}
