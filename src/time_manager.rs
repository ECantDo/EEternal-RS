use crate::search::search_types::{SearchData, SharedData};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub enum Limits {
    Infinite,
    Depth(i32),
    Time(u64),
    Nodes(u64),
    Mate(u64),
    Fischer(u64, u64),
    Cyclic(u64, u64, u64),
}

const TIME_OVERHEAD_MS: u64 = 1;

#[derive(Clone)]
pub struct TimeManager {
    limits: Limits,
    start_time: Instant,
    soft_bound: Duration,
    hard_bound: Duration,
}

impl TimeManager {
    pub fn new(limits: Limits, fullmove_number: usize, move_overhead: u64) -> Self {
        let soft;
        let hard;

        match limits {
            // go movetime
            Limits::Time(ms) => {
                soft = ms;
                hard = ms;
            }
            // Standard tc
            Limits::Fischer(main, inc) => {
                let inc_time: f64 = 0.70 * inc as f64;

                let soft_scale = 0.0594 - 0.0492 * (-0.0386 * fullmove_number as f64).exp();
                let hard_scale: f64 = 0.30;

                let soft_bound =
                    (soft_scale * main.saturating_sub(move_overhead) as f64 + inc_time) as u64;
                let hard_bound =
                    (hard_scale * main.saturating_sub(move_overhead) as f64 + inc_time) as u64;

                // Don't think for longer than what I have left
                soft = soft_bound.min(main.saturating_sub(move_overhead));
                hard = hard_bound.min(main.saturating_sub(move_overhead));
            }
            // <x> moves in <y> time
            Limits::Cyclic(main, inc, moves) => {
                let main = main.saturating_sub(move_overhead);
                let base = (main as f64 / moves as f64) + 0.75 * inc as f64;
                soft = ((1.0 * base) as u64).min(main + inc);
                hard = ((5.0 * base) as u64).min(main + inc);
            }
            _ => {
                soft = u64::MAX;
                hard = u64::MAX;
            }
        }

        Self {
            limits,
            start_time: Instant::now(),
            soft_bound: Duration::from_millis(soft.saturating_sub(TIME_OVERHEAD_MS)),
            hard_bound: Duration::from_millis(hard.saturating_sub(TIME_OVERHEAD_MS)),
        }
    }

    pub fn soft_limit_exceeded(&self, shared_data: &SharedData) -> bool {
        match self.limits {
            Limits::Infinite | Limits::Depth(_) | Limits::Mate(_) => false,
            Limits::Nodes(max) => shared_data.nodes.count() >= max,
            Limits::Time(max) => self.start_time.elapsed() >= Duration::from_millis(max),
            _ => self.start_time.elapsed() >= self.soft_bound,
        }
    }

    pub fn check_time(&self, search_data: &SearchData) -> bool {
        if search_data.completed_depth == 0 {
            return false;
        }

        match self.limits {
            Limits::Infinite | Limits::Depth(_) | Limits::Mate(_) => false,
            Limits::Nodes(maximum) => search_data.shared_data.nodes.count() > maximum,
            // Don't constantly check time ... let it search ~2k nodes first
            _ => search_data.nodes() & 2047 == 2047 && self.start_time.elapsed() >= self.hard_bound,
        }
    }

    pub fn reset_start(&mut self) {
        self.start_time = Instant::now();
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }
}
