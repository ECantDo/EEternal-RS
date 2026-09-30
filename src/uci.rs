use crate::{
    board::{Board, generate_moves::AllMoves},
    search::{
        search_types::{SearchData, SharedData},
        start_search,
    },
    settings::Settings,
    time_manager::{Limits, TimeManager},
    types::{
        color::Color,
        move_list::MoveEntry,
        {piece::PieceType, square::Square},
    },
};
use std::{
    io::{self, BufRead, Write},
    sync::{Arc, atomic::Ordering},
};
use crate::search::PrintOutput;

pub fn run_uci() {
    let stdin = io::stdin();
    let mut board = Board::startpos();
    let mut shared_data: Arc<SharedData> = Arc::new(SharedData::new());

    // Holds the SearchData when no search thread owns it.
    let mut search_data: Option<SearchData> = Some(SearchData::new(Arc::clone(&shared_data)));
    let mut search_thread: Option<std::thread::JoinHandle<SearchData>> = None;

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
                // println!("option name EvalFile type string default <empty>");
                println!(
                    "option name Hash type spin default {} min {} max {}",
                    Settings::DEFAULT_TT_SIZE_MB,
                    Settings::TT_MIN_SIZE,
                    Settings::TT_MAX_SIZE
                );
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "ucinewgame" => {
                board = Board::startpos();
                if let Some(handle) = search_thread.take() {
                    shared_data.stop.store(true, Ordering::Relaxed);
                    let _ = handle.join();
                }
                // Fresh history/killers/PV for a new game.
                search_data = Some(SearchData::new(Arc::clone(&shared_data)));
            }
            "position" => handle_position(&mut board, rest),
            "go" => {
                if let Some(handle) = search_thread.take() {
                    shared_data.stop.store(true, Ordering::Relaxed);
                    if let Ok(returned) = handle.join() {
                        search_data = Some(returned);
                    }
                }
                shared_data.stop.store(false, Ordering::Relaxed);
                shared_data.nodes.reset();

                let sd = search_data
                    .take()
                    .expect("search_data missing — a search must still be running");
                let board_clone = board.clone();
                let rest_owned = rest.to_string();

                search_thread = Some(std::thread::spawn(move || {
                    handle_go(board_clone, &rest_owned, sd)
                }));
            }
            "setoption" => {
                if let Some(handle) = search_thread.take() {
                    shared_data.stop.store(true, Ordering::Relaxed);
                    let _ = handle.join();
                }
                // SearchData holds an Arc clone, so drop it for the Hash resize (Arc::get_mut)
                search_data = None;
                handle_setoption(rest, &mut shared_data);
                search_data = Some(SearchData::new(Arc::clone(&shared_data)));
            }
            "stop" => shared_data.stop.store(true, Ordering::Relaxed),
            "quit" => {
                shared_data.stop.store(true, Ordering::Relaxed);
                break;
            }
            "d" => println!("{board}"),
            "bench" => {
                let args: Vec<&str> = rest.split_whitespace().collect();
                crate::utils::bench::bench::<true>(&args);
            }
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

fn handle_go(mut board: Board, rest: &str, mut search_data: SearchData) -> SearchData {
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
        return search_data;
    }

    let limits = parse_limits(board.side_to_move(), &*parts);

    if matches!(limits, Limits::Infinite) {
        return search_data; // Can do inf, no way to stop
    }

    search_data.set_board(&board);
    search_data.new_search();
    // TODO : Figure out overhead (guessing 15ms)
    search_data.time_manager = TimeManager::new(limits, board.full_move_number(), 15);

    let mv = start_search::<PrintOutput>(&mut search_data);
    println!("bestmove {}", mv.to_uci());
    io::stdout().flush().unwrap();

    search_data
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

    let legal = board.generate_all_legal_moves::<AllMoves>();
    legal.into_iter().find(|move_entry| {
        move_entry.mv().from() == from
            && move_entry.mv().to() == to
            && (!move_entry.mv().is_promotion()
                || Some(move_entry.mv().promotion_piece_type()) == promo)
    })
}

fn handle_setoption(rest: &str, shared_data: &mut Arc<SharedData>) {
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
    } else if name.eq_ignore_ascii_case("Hash") {
        match Arc::get_mut(shared_data) {
            Some(data) => {
                let parsed_value: usize = value.parse().unwrap_or(Settings::DEFAULT_TT_SIZE_MB);
                if parsed_value < Settings::TT_MIN_SIZE || parsed_value > Settings::TT_MAX_SIZE {
                    eprintln!("Invalid tt size");
                    return;
                }
                data.resize_tt(parsed_value);
                println!("info string resized TT to {} mb", parsed_value);
            }
            None => eprintln!("Cannot operate on `shared_data` at this time."),
        }
    }
}
