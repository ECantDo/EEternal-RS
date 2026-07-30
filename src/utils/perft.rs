use crate::{board::{Board, generate_moves::*}};

use std::time::Instant;

struct PerftInputs {
    fen: &'static str,
    expected_results: &'static [u64],
    depth: u8,
}

const PERFT_TESTS: &[PerftInputs] = &[
    PerftInputs {
        fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        expected_results: &[1, 20, 400, 8902, 197281, 4865609, 119060324],
        depth: 5,
    },
    PerftInputs {
        fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - ",
        expected_results: &[1, 48, 2039, 97862, 4085603, 193690690, 8031647685],
        depth: 4,
    },
    PerftInputs {
        fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1 ",
        expected_results: &[
            1, 14, 191, 2812, 43238, 674624, 11030083, 178633661, 3009794393,
        ],
        depth: 6,
    },
    PerftInputs {
        fen: "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        expected_results: &[1, 6, 264, 9467, 422333, 15833292, 706045033, 27209691363],
        depth: 5,
    },
    PerftInputs {
        fen: "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        expected_results: &[1, 44, 1486, 62379, 2103487, 89941194],
        depth: 4,
    },
    PerftInputs {
        fen: "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        expected_results: &[
            1,
            46,
            2079,
            89890,
            3894594,
            164075551,
            6923051137,
            287188994746,
            11923589843526,
            490154852788714,
        ],
        depth: 4,
    },
];

impl Board {
    pub fn perft_bulk(&mut self, depth: usize) -> u64 {
        if depth == 0 {
            return 1;
        }

        let ml = self.generate_all_legal_moves::<AllMoves>();

        if depth == 1 {
            return ml.len() as u64;
        }

        let mut nodes = 0;
        for move_entry in &ml {
            self.make_move(move_entry.mv());
            nodes += self.perft_bulk(depth - 1);
            self.undo_move(move_entry.mv());
        }
        nodes
    }
    pub fn perft(&mut self, depth: usize) -> u64 {
        if depth == 0 {
            return 1;
        }

        let ml = self.generate_all_legal_moves::<AllMoves>();

        let mut nodes = 0;
        for move_entry in &ml {
            self.make_move(move_entry.mv());
            nodes += self.perft(depth - 1);
            self.undo_move(move_entry.mv());
        }
        nodes
    }
}

pub fn perft_test() {
    println!("=====> Running full perft test");
    let test_start = Instant::now();
    let mut total: u64 = 0;
    for test in PERFT_TESTS {
        let mut board = Board::parse_fen_string(test.fen).unwrap();
        let perft_start = Instant::now();
        let result = board.perft(test.depth as usize);
        let duration = perft_start.elapsed().as_millis();
        total += result;
        let nps = (result as u128) * 1000 / duration;
        let success = if result == test.expected_results[test.depth as usize] {
            "[PASS]"
        } else {
            &*format!("[FAIL] {}", test.expected_results[test.depth as usize])
        };
        println!(
            "Result: {} \t Depth: {} \t Time: {:?} ms \t nps: {} \t {}",
            result, test.depth, duration, nps, success
        );
    }
    let duration = test_start.elapsed().as_millis();
    let nps = (total as u128) * 1000 / duration;
    println!("Total Time: {:?} ms\nTotal nps: {}", duration, nps);
}
