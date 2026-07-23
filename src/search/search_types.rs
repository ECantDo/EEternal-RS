use crate::nnue::NNUE;
use crate::time_manager::Limits;
use crate::types::tt::TranspositionTable;
use crate::types::MAX_PLY;
use crate::{
    board::Board,
    time_manager::TimeManager,
    types::{moves::Move, score::Score},
};
use std::sync::atomic::AtomicBool;
use std::sync::{
    atomic::{AtomicU64, Ordering}, Arc,
    LazyLock,
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
    pub stop: AtomicBool,
    pub tt: TranspositionTable,
}
pub struct SearchData {
    pub board: Board,
    pub completed_depth: usize,
    pub nnue: NNUE,
    pub root_move: RootMove,
    pub shared_data: Arc<SharedData>,
    pub time_manager: TimeManager,
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
        let board = Board::startpos();
        let mut nnue = NNUE::new();
        nnue.reset(&board);
        Self {
            board,
            completed_depth: 0,
            nnue,
            root_move: RootMove::default(),
            shared_data: shared,
            time_manager: TimeManager::new(Limits::Infinite, 0, 0),
        }
    }

    pub fn evaluate(&mut self) -> i32 {
        // return self.board.evaluate();
        if self.nnue.is_active() {
            self.nnue.evaluate(&self.board)
        } else {
            self.board.evaluate()
        }
    }

    pub fn make_move(&mut self, mv: Move) {
        self.nnue.push(mv, &self.board);
        self.board.make_move(mv);
    }

    pub fn undo_move(&mut self, mv: Move) {
        self.board.undo_move(mv);
        self.nnue.pop();
    }

    pub fn set_board(&mut self, board: &Board) {
        self.board = board.clone();
    }

    pub fn nodes(&self) -> u64 {
        self.shared_data.nodes.count()
    }

    pub fn reset_clock(&mut self) {
        self.time_manager.reset_start();
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.time_manager.elapsed().as_millis()
    }

    pub fn elapsed_us(&self) -> u128 {
        self.time_manager.elapsed().as_micros()
    }

    pub fn elapsed_ns(&self) -> u128 {
        self.time_manager.elapsed().as_nanos()
    }

    pub fn to_uci_info(&self) -> String {
        let time_ms = self.elapsed_ms();
        let nodes = self.nodes();
        let nps: u128 = (nodes as u128) * 1_000_000 / self.elapsed_us().max(1);

        let mut uci_out = String::from(format!("info depth {} ", self.completed_depth));

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

        uci_out += format!("hashfull {} ", self.shared_data.tt.hashfull()).as_str();

        uci_out
    }
}

impl SharedData {
    pub fn new() -> Self {
        Self {
            nodes: Counter::default(),
            stop: AtomicBool::new(false),
            tt: TranspositionTable::new(16),
        }
    }
}

pub static LMR_TABLE: LazyLock<[[i32; MAX_PLY]; MAX_PLY]> = LazyLock::new(|| {
    std::array::from_fn(|depth| {
        std::array::from_fn(|moves| lmr_reduction(depth as i32, moves as i32))
    })
});
fn lmr_reduction(depth: i32, moves_searched: i32) -> i32 {
    (0.75 + (depth.max(1) as f64).ln() * (moves_searched.max(1) as f64).ln() / 2.5).max(0.0) as i32
}
