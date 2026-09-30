use crate::types::MAX_PLY;

pub type ScoreType = i32;
pub struct Score;

impl Score {
	pub const ZERO: ScoreType = 0;
	pub const NONE: ScoreType = 32002;
	pub const INF: ScoreType = 32001;
	pub const MATE: ScoreType = 32000;

	pub const MATE_IN_MAX: ScoreType = Self::MATE - MAX_PLY as ScoreType;

	pub const fn mated_in(ply: i32) -> ScoreType {
		(ply as ScoreType) - Self::MATE
	}

	pub fn score_to_mate_moves(score: ScoreType) -> ScoreType {
		let dst: ScoreType = Self::MATE - score.abs();
		let moves: ScoreType = (dst + 1) / 2;
		if score > 0 { moves } else { -moves }
	}
}
