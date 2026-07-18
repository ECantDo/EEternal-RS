use crate::types::score::Score;
use crate::{board::Board, types::moves::Move};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};

#[derive(Clone)]
pub struct RootMove {
    pub mv: Move,
    pub score: i32,
}

impl Default for RootMove {
    fn default() -> Self {
        Self {
            mv: Move::NONE,
            score: Score::NONE,
        }
    }
}

pub struct SharedData {
    // tt
    pub nodes: Counter,
}
pub struct SearchData {
    pub board: Board,
    pub shared_data: Arc<SharedData>,
    pub start_time: Instant,
    pub root_move: RootMove,
}

pub struct Counter {
    count: AtomicU64,
}

impl Counter {
    pub fn increment(&self) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add(&self, val: u64) {
        self.count.fetch_add(val, Ordering::Relaxed);
    }

    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self {
            count: AtomicU64::default(),
        }
    }
}

impl SearchData {
    pub fn new(shared: Arc<SharedData>) -> Self {
        Self {
            board: Board::startpos(),
            shared_data: shared,
            start_time: Instant::now(),
            root_move: RootMove::default(),
        }
    }

    pub fn set_board(&mut self, board: &Board) {
        self.board = board.clone();
    }

    pub fn nodes(&self) -> u64 {
        self.shared_data.nodes.count()
    }

    pub fn reset_clock(&mut self) {
        self.start_time = Instant::now();
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.start_time.elapsed().as_millis()
    }

    pub fn elapsed_us(&self) -> u128 {
        self.start_time.elapsed().as_micros()
    }

    pub fn elapsed_ns(&self) -> u128 {
        self.start_time.elapsed().as_nanos()
    }

    pub fn to_uci_info(&self, depth: i32) -> String {
        let time_ms = self.elapsed_ms();
        let nodes = self.nodes();
        let nps: u128 = (nodes as u128) * 1_000_000 / self.elapsed_us();

        let mut uci_out = String::from(format!("info depth {} ", depth));

        if self.root_move.score.abs() >= Score::MATE_IN_MAX {
            uci_out += format!(
                "score mate {} ",
                Score::score_to_mate_moves(self.root_move.score)
            )
            .as_str();
        } else {
            uci_out += format!("score cp {} ", self.root_move.score).as_str();
        }

        uci_out += format!("nodes {} ", nodes).as_str();

        uci_out += format!("time {} ", time_ms).as_str();

        uci_out += format!("nps {} ", nps).as_str();

        uci_out
    }
}

impl SharedData {
    pub fn new() -> Self {
        Self {
            nodes: Counter::default(),
        }
    }
}
