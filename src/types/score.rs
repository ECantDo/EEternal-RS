use crate::types::MAX_PLY;

pub struct Score;

impl Score {
    pub const ZERO: i32 = 0;
    pub const NONE: i32 = 64002;
    pub const INF: i32 = 64001;
    pub const MATE: i32 = 64000;

    pub const MATE_IN_MAX: i32 = 32000 - MAX_PLY as i32;

    pub const fn mated_in(ply: i32) -> i32 {
        ply - Self::MATE
    }
}
