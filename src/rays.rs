// rays.rs — lives next to attacking.rs (needs its magic-bitboard lookups,
// so it can't live inside `types`, same as board.rs).
use std::sync::LazyLock;
use crate::{
    attacking::{get_bishop_attacks, get_rook_attacks},
    types::{bitboard::Bitboard, square::Square},
};

static BETWEEN: LazyLock<[[Bitboard; 64]; 64]> = LazyLock::new(|| {
    let mut table = [[Bitboard(0); 64]; 64];
    for a in 0..64u8 {
        let sa = Square::new(a);
        for b in 0..64u8 {
            if a == b { continue; }
            let sb = Square::new(b);
            table[a as usize][b as usize] =
                if get_rook_attacks(sa, Bitboard(0)).contains(sb) {
                    get_rook_attacks(sa, sb.to_bitboard()) & get_rook_attacks(sb, sa.to_bitboard())
                } else if get_bishop_attacks(sa, Bitboard(0)).contains(sb) {
                    get_bishop_attacks(sa, sb.to_bitboard()) & get_bishop_attacks(sb, sa.to_bitboard())
                } else {
                    Bitboard(0)
                };
        }
    }
    table
});

/// Full line through both squares (endpoints included), extended to the
/// board edge in both directions. Used to check "does this pinned piece's
/// destination stay on the king-pinner line".
static LINE: LazyLock<[[Bitboard; 64]; 64]> = LazyLock::new(|| {
    let mut table = [[Bitboard(0); 64]; 64];
    for a in 0..64u8 {
        let sa = Square::new(a);
        for b in 0..64u8 {
            if a == b { continue; }
            let sb = Square::new(b);
            table[a as usize][b as usize] =
                if get_rook_attacks(sa, Bitboard(0)).contains(sb) {
                    (get_rook_attacks(sa, Bitboard(0)) & get_rook_attacks(sb, Bitboard(0)))
                        | sa.to_bitboard() | sb.to_bitboard()
                } else if get_bishop_attacks(sa, Bitboard(0)).contains(sb) {
                    (get_bishop_attacks(sa, Bitboard(0)) & get_bishop_attacks(sb, Bitboard(0)))
                        | sa.to_bitboard() | sb.to_bitboard()
                } else {
                    Bitboard(0)
                };
        }
    }
    table
});

pub fn between(a: Square, b: Square) -> Bitboard {
    BETWEEN[a as usize][b as usize]
}

pub fn line_through(a: Square, b: Square) -> Bitboard {
    LINE[a as usize][b as usize]
}