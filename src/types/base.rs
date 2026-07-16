// Likely needs to be changed, just some value for now
pub const MAX_PLY: usize = 256;

// Max moves in a position is 218; memory-wise 256 makes sense to me
pub const MAX_MOVES: usize = 256;

#[repr(u8)]
#[derive(Clone, PartialEq, PartialOrd)]
pub enum Rank {
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
}

pub const PROMOTION_RANK: [Rank; 2] = [Rank::R8, Rank::R1];
pub const HOME_RANK: [Rank; 2] = [Rank::R1, Rank::R8];

#[repr(u8)]
#[derive(Clone, PartialEq, PartialOrd)]
pub enum File {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}
