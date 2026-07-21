use crate::types::MAX_PLY;

pub struct Score;

impl Score {
    pub const ZERO: i32 = 0;
    pub const NONE: i32 = 32002;
    pub const INF: i32 = 32001;
    pub const MATE: i32 = 32000;

    pub const MATE_IN_MAX: i32 = Self::MATE - MAX_PLY as i32;

    pub const fn mated_in(ply: i32) -> i32 {
        ply - Self::MATE
    }

    pub fn score_to_mate_moves(score: i32) -> i32 {
        let dst: i32 = Self::MATE - score.abs();
        let moves: i32 = (dst + 1) / 2;
        if score > 0 { moves } else { -moves }
    }
}
