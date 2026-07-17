mod generate_tables;
mod magic_numbers;

use crate::types::{bitboard::Bitboard, square::Square};
use generate_tables::{bishop_attacks_slow, rook_attacks_slow};
pub use magic_numbers::{BISHOP_MAGICS, BISHOP_MAP_SIZE, MagicEntry, ROOK_MAGICS, ROOK_MAP_SIZE};
use std::sync::LazyLock;

static ROOK_TABLE: LazyLock<Vec<Bitboard>> =
    LazyLock::new(|| build_table(&ROOK_MAGICS, ROOK_MAP_SIZE, rook_attacks_slow));

static BISHOP_TABLE: LazyLock<Vec<Bitboard>> =
    LazyLock::new(|| build_table(&BISHOP_MAGICS, BISHOP_MAP_SIZE, bishop_attacks_slow));

fn build_table(
    magics: &[MagicEntry; 64],
    map_size: usize,
    slow_attacks: fn(Square, Bitboard) -> Bitboard,
) -> Vec<Bitboard> {
    let mut table = vec![Bitboard(0); map_size];

    for sq_idx in 0..64 {
        let square = Square::new(sq_idx as u8);
        let entry = &magics[sq_idx];
        let bits = entry.mask.count_ones();

        for i in 0..(1u32 << bits) {
            let occ = index_to_occupancy(i, Bitboard(entry.mask));
            let attacks = slow_attacks(square, occ);
            let index = ((occ.0 & entry.mask).wrapping_mul(entry.magic) >> entry.shift) as usize;
            table[entry.offset + index] = attacks;
        }
    }

    table
}

fn index_to_occupancy(index: u32, mask: Bitboard) -> Bitboard {
    let mut occ = 0u64;
    let mut m = mask.0;
    let mut i = 0;
    while m != 0 {
        let sq = m.trailing_zeros();
        m &= m - 1;
        if (index >> i) & 1 != 0 {
            occ |= 1 << sq;
        }
        i += 1;
    }
    Bitboard(occ)
}

pub fn rook_attacks(square: Square, occ: Bitboard) -> Bitboard {
    ROOK_TABLE[magic_index(occ.0, &ROOK_MAGICS[square as usize])]
}

pub fn bishop_attacks(square: Square, occ: Bitboard) -> Bitboard {
    BISHOP_TABLE[magic_index(occ.0, &BISHOP_MAGICS[square as usize])]
}

const fn magic_index(occupancies: u64, entry: &MagicEntry) -> usize {
    let mut hash = occupancies & entry.mask;
    hash = hash.wrapping_mul(entry.magic) >> entry.shift;
    hash as usize + entry.offset
}

#[test]
fn magic_tables_match_slow_attacks() {
    for sq_idx in 0..64u8 {
        let sq = Square::new(sq_idx);
        // check a handful of occupancy patterns, e.g. empty board and full board
        for occ_raw in [0u64, u64::MAX, 0x0000_FFFF_0000_FFFF] {
            let occ = Bitboard(occ_raw);
            assert_eq!(rook_attacks(sq, occ), generate_tables::rook_attacks_slow(sq, occ));
            assert_eq!(bishop_attacks(sq, occ), generate_tables::bishop_attacks_slow(sq, occ));
        }
    }
}