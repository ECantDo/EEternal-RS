use crate::types::{bitboard::Bitboard, square::Square};

pub fn rook_attacks_slow(sq: Square, occ: Bitboard) -> Bitboard {
    let rank = sq as i32 / 8;
    let file = sq as i32 % 8;
    let mut attacks = 0u64;

    for r in (rank + 1)..8 {
        let s = (r * 8 + file) as u8;
        attacks |= 1 << s;
        if occ.contains(Square::new(s)) {
            break;
        }
    }
    for r in (0..rank).rev() {
        let s = (r * 8 + file) as u8;
        attacks |= 1 << s;
        if occ.contains(Square::new(s)) {
            break;
        }
    }
    for f in (file + 1)..8 {
        let s = (rank * 8 + f) as u8;
        attacks |= 1 << s;
        if occ.contains(Square::new(s)) {
            break;
        }
    }
    for f in (0..file).rev() {
        let s = (rank * 8 + f) as u8;
        attacks |= 1 << s;
        if occ.contains(Square::new(s)) {
            break;
        }
    }

    Bitboard(attacks)
}

pub fn bishop_attacks_slow(sq: Square, occ: Bitboard) -> Bitboard {
    let rank = sq as i32 / 8;
    let file = sq as i32 % 8;
    let mut attacks = 0u64;

    for (dr, df) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
        let mut r = rank + dr;
        let mut f = file + df;
        while (0..8).contains(&r) && (0..8).contains(&f) {
            let s = (r * 8 + f) as u8;
            attacks |= 1 << s;
            if occ.contains(Square::new(s)) {
                break;
            }
            r += dr;
            f += df;
        }
    }

    Bitboard(attacks)
}
