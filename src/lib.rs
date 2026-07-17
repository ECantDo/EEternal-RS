pub mod board;

pub mod types;

pub mod magic_utils;

use crate::types::{bitboard::Bitboard, square::Square};
use magic_utils::{bishop_attacks, rook_attacks};


pub fn initialize() {
    println!("Initializing chess engine tables...");
    let _ = rook_attacks(Square::A1, Bitboard(0));
    let _ = bishop_attacks(Square::A1, Bitboard(0));
    println!("Tables initialized");
    // Wowww... amazing ... there is so much here... ;-;
}

