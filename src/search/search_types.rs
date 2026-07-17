use crate::board::Board;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

pub struct SharedData {
    // tt
    pub nodes: Counter,
}
pub struct SearchData {
    pub board: Board,
    pub shared_data: Arc<SharedData>,
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
        }
    }

    pub fn nodes(&self) -> u64 {
        self.shared_data.nodes.count()
    }
}

impl SharedData {
    pub fn new() -> Self {
        Self {
            nodes: Counter::default(),
        }
    }
}
