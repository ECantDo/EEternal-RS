pub mod board;

pub mod types;

pub mod attacking;

use crate::types::{bitboard::Bitboard, square::Square};

pub fn initialize() {
    attacking::initialize_lookups();
    // Wowww... amazing ... there is so much here... ;-;
}
