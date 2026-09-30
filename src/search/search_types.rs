use crate::settings::Settings;
use crate::types::score::ScoreType;
use crate::{
    board::Board,
    history::{NoisyHistory, QuietHistory},
    nnue::NNUE,
    time_manager::Limits,
    time_manager::TimeManager,
    types::{MAX_PLY, moves::Move, score::Score, tt::TranspositionTable},
};
use std::ops::{Index, IndexMut};
use std::sync::{
    Arc, LazyLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

// =================================================================================================
// Root Move
// =================================================================================================
#[derive(Clone)]
pub struct RootMove {
    pub mv: Move,
    pub pv: RootPV,
    pub score: i32,
    pub previous_score: i32,
}

impl Default for RootMove {
    fn default() -> Self {
        Self {
            mv: Move::NONE,
            pv: RootPV::default(),
            score: Score::NONE,
            previous_score: Score::NONE,
        }
    }
}

// =================================================================================================
// Shared Data
// =================================================================================================
pub struct SharedData {
    // tt
    pub nodes: Counter,
    pub stop: AtomicBool,
    pub tt: TranspositionTable,
}

impl SharedData {
    pub fn new() -> Self {
        Self {
            nodes: Counter::default(),
            stop: AtomicBool::new(false),
            tt: TranspositionTable::new(Settings::DEFAULT_TT_SIZE_MB),
        }
    }

    pub fn resize_tt(&mut self, tt_size: usize) {
        self.tt = TranspositionTable::new(tt_size);
    }
}

// =================================================================================================
// Search Data
// =================================================================================================
pub struct SearchData {
    pub board: Board,
    pub completed_depth: usize,
    pub killers: Killers,
    pub nnue: NNUE,
    pub noisy_history: NoisyHistory,
    pub pv_table: PrincipalVariationTable,
    pub quiet_history: QuietHistory,
    pub root_move: RootMove,
    pub shared_data: Arc<SharedData>,
    pub time_manager: TimeManager,
    pub stack: SearchStack,
}

impl SearchData {
    pub fn new(shared: Arc<SharedData>) -> Self {
        let board = Board::startpos();
        let mut nnue = NNUE::new();
        nnue.reset(&board);
        Self {
            board,
            completed_depth: 0,
            killers: Killers::default(),
            nnue,
            noisy_history: NoisyHistory::default(),
            pv_table: PrincipalVariationTable::default(),
            quiet_history: QuietHistory::default(),
            root_move: RootMove::default(),
            shared_data: shared,
            time_manager: TimeManager::new(Limits::Infinite, 0, 0),
            stack: SearchStack::default(),
        }
    }

    pub fn new_search(&mut self) {
        self.completed_depth = 0;
        self.killers = Killers::default();
        self.nnue.reset(&self.board);
        self.pv_table.clear(0);
        self.root_move = RootMove::default();
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
            uci_out.push_str(&format!(
                "score mate {} ",
                Score::score_to_mate_moves(self.root_move.score)
            ));
        } else {
            uci_out.push_str(&format!("score cp {} ", self.root_move.score));
        }

        uci_out.push_str(&format!("nodes {} ", nodes));

        uci_out.push_str(&format!("time {} ", time_ms));

        uci_out.push_str(&format!("nps {} ", nps));

        uci_out.push_str(&format!("hashfull {} ", self.shared_data.tt.hashfull()));

        uci_out.push_str(&format!("pv {}", self.root_move.mv.to_uci()));
        for mv in self.root_move.pv.line() {
            uci_out.push_str(&format!(" {}", mv.to_uci()));
        }

        uci_out
    }
}

// =================================================================================================
// Counter
// =================================================================================================
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

// =================================================================================================
// LMR
// =================================================================================================
pub static LMR_TABLE: LazyLock<[[i32; MAX_PLY]; MAX_PLY]> = LazyLock::new(|| {
    std::array::from_fn(|depth| {
        std::array::from_fn(|moves| lmr_reduction(depth as i32, moves as i32))
    })
});
fn lmr_reduction(depth: i32, moves_searched: i32) -> i32 {
    (0.75 + (depth.max(1) as f64).ln() * (moves_searched.max(1) as f64).ln() / 2.5).max(0.0) as i32
}

// =================================================================================================
// PV Line
// =================================================================================================
#[derive(Clone)]
pub struct PrincipalVariationTable {
    table: Box<[[Move; MAX_PLY + 1]]>,
    len: [usize; MAX_PLY + 1],
}

impl PrincipalVariationTable {
    pub const fn clear(&mut self, ply: usize) {
        self.len[ply] = 0;
    }

    pub fn update(&mut self, ply: usize, mv: Move) {
        self.table[ply][0] = mv;
        self.len[ply] = self.len[ply + 1] + 1;

        for i in 0..self.len[ply + 1] {
            self.table[ply][i + 1] = self.table[ply + 1][i];
        }
    }
}

impl Default for PrincipalVariationTable {
    fn default() -> Self {
        Self {
            table: vec![[Move::NONE; MAX_PLY + 1]; MAX_PLY + 1].into_boxed_slice(),
            len: [0; MAX_PLY + 1],
        }
    }
}

#[derive(Clone)]
pub struct RootPV {
    table: Box<[Move; MAX_PLY + 1]>,
    len: usize,
}

impl RootPV {
    pub fn line(&self) -> &[Move] {
        &self.table[..self.len]
    }

    pub fn commit_from_pv(&mut self, src: &PrincipalVariationTable, ply: usize) {
        let len = src.len[ply].min(MAX_PLY + 1);
        self.len = len;
        self.table[..len].copy_from_slice(&src.table[ply][..len]);
    }
}

impl Default for RootPV {
    fn default() -> Self {
        Self {
            table: Box::new([Move::NONE; MAX_PLY + 1]),
            len: 0,
        }
    }
}
// =================================================================================================
// Killers
// =================================================================================================
pub struct Killers {
    killers: [[Move; 2]; MAX_PLY],
}

impl Killers {
    pub fn append(&mut self, ply: i32, mv: Move) {
        debug_assert!(ply < MAX_PLY as i32);

        let death = &mut self.killers[ply as usize];
        if death[0] == mv {
            return;
        }

        death[1] = death[0];
        death[0] = mv;
    }

    pub fn get(&self, ply: i32) -> [Move; 2] {
        debug_assert!(ply < MAX_PLY as i32);
        self.killers[ply as usize]
    }
}

impl Default for Killers {
    fn default() -> Self {
        Self {
            killers: [[Move::NONE; 2]; MAX_PLY],
        }
    }
}

// =================================================================================================
// Search Stack
// =================================================================================================
/// Per-ply data for the current search path, indexed by `ply` (as `i32`).
///
/// The first `STACK_PADDING` entries sit *before* ply 0, so looking back from near the root
/// (e.g. `stack[ply - 4]` at ply 1) lands on a default entry instead of going out of bounds.
/// Those padding entries are never written to, so they always hold their defaults (e.g.
/// `Score::NONE` for `static_eval`), which reads as "no data".
const STACK_PADDING: usize = 8;

pub struct SearchStack {
    data: [StackEntry; MAX_PLY + STACK_PADDING + 8],
}

#[derive(Copy, Clone)]
pub struct StackEntry {
    /// `Score::NONE` when there is no static eval (in check, or not searched yet)
    pub static_eval: ScoreType,
}

impl Default for StackEntry {
    fn default() -> Self {
        Self {
            static_eval: Score::NONE,
        }
    }
}

impl Default for SearchStack {
    fn default() -> Self {
        Self {
            data: [StackEntry::default(); MAX_PLY + STACK_PADDING + 8],
        }
    }
}

impl Index<i32> for SearchStack {
    type Output = StackEntry;

    fn index(&self, ply: i32) -> &Self::Output {
        debug_assert!(ply + STACK_PADDING as i32 >= 0);
        &self.data[(ply + STACK_PADDING as i32) as usize]
    }
}

impl IndexMut<i32> for SearchStack {
    fn index_mut(&mut self, ply: i32) -> &mut Self::Output {
        debug_assert!(ply + STACK_PADDING as i32 >= 0);
        &mut self.data[(ply + STACK_PADDING as i32) as usize]
    }
}
