mod magic_utils;
mod not_magical;

use crate::{
    attacking::not_magical::{king_attacks, knight_attacks, pawn_attacks},
    types::{bitboard::Bitboard, color::Color, square::Square},
};
use magic_utils::{bishop_attacks, rook_attacks};

// These are existing just so that I don't have to get things from different locations
pub fn get_king_attacks(square: Square) -> Bitboard {
    king_attacks(square)
}

pub fn get_pawn_attacks(square: Square, color: Color) -> Bitboard {
    pawn_attacks(square, color)
}

pub fn get_knight_attacks(square: Square) -> Bitboard {
    knight_attacks(square)
}

pub fn get_bishop_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    bishop_attacks(square, occupancy)
}

pub fn get_rook_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    rook_attacks(square, occupancy)
}

pub fn get_queen_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    bishop_attacks(square, occupancy) | rook_attacks(square, occupancy)
}

pub fn initialize_lookups() {
    let _ = rook_attacks(Square::A1, Bitboard(0));
    let _ = bishop_attacks(Square::A1, Bitboard(0));
    let _ = king_attacks(Square::A1);
    let _ = knight_attacks(Square::A1);
    let _ = pawn_attacks(Square::A2, Color::White); // Just to be safe, A2, since pawns cant be on A1
}
