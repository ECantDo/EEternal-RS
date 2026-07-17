use crate::types::{bitboard::Bitboard, color::Color, square::Square};
use std::sync::LazyLock;

static KNIGHT_ATTACKS: LazyLock<[Bitboard; 64]> = LazyLock::new(|| {
    let mut table = [Bitboard(0); 64];
    for sq in 0..64u8 {
        table[sq as usize] = knight_attacks_slow(Square::new(sq));
    }
    table
});

static KING_ATTACKS: LazyLock<[Bitboard; 64]> = LazyLock::new(|| {
    let mut table = [Bitboard(0); 64];
    for sq in 0..64u8 {
        table[sq as usize] = king_attacks_slow(Square::new(sq));
    }
    table
});

static PAWN_ATTACKS: LazyLock<[[Bitboard; 64]; 2]> = LazyLock::new(|| {
    let mut table = [[Bitboard(0); 64]; 2];
    for sq in 0..64u8 {
        table[Color::White as usize][sq as usize] =
            pawn_attacks_slow(Square::new(sq), Color::White);
        table[Color::Black as usize][sq as usize] =
            pawn_attacks_slow(Square::new(sq), Color::Black);
    }
    table
});

pub fn knight_attacks(sq: Square) -> Bitboard {
    KNIGHT_ATTACKS[sq as usize]
}

pub fn king_attacks(sq: Square) -> Bitboard {
    KING_ATTACKS[sq as usize]
}

pub fn pawn_attacks(sq: Square, color: Color) -> Bitboard {
    PAWN_ATTACKS[color as usize][sq as usize]
}

fn knight_attacks_slow(sq: Square) -> Bitboard {
    let rank: i8 = sq as i8 / 8; // i8 is fine ... 0..63 is not 7-bits (0..127)
    let file: i8 = sq as i8 % 8;
    let mut attacks = Bitboard(0);

    for (dr, df) in [
        (-2i8, -1i8),
        (-2, 1),
        (-1, -2),
        (-1, 2),
        (1, -2),
        (1, 2),
        (2, -1),
        (2, 1),
    ] {
        let r = rank + dr;
        let f = file + df;
        if (0..8).contains(&r) && (0..8).contains(&f) {
            attacks |= Square::get_board_square(r as u8, f as u8).to_bitboard();
        }
    }

    attacks
}

fn king_attacks_slow(sq: Square) -> Bitboard {
    let rank = sq as i8 / 8;
    let file = sq as i8 % 8;
    let mut attacks = Bitboard(0);

    for dr in -1..=1 {
        for df in -1..=1 {
            if dr == 0 && df == 0 {
                continue;
            }
            let r = rank + dr;
            let f = file + df;
            if (0..8).contains(&r) && (0..8).contains(&f) {
                attacks |= Square::get_board_square(r as u8, f as u8).to_bitboard();
            }
        }
    }

    attacks
}

fn pawn_attacks_slow(sq: Square, color: Color) -> Bitboard {
    let rank = sq as i8 / 8;
    let file = sq as i8 % 8;
    let dir = if color == Color::White { 1 } else { -1 };
    let mut attacks = Bitboard(0);

    for df in [-1, 1] {
        let r = rank + dir;
        let f = file + df;
        if (0..8).contains(&r) && (0..8).contains(&f) {
            attacks |= Square::get_board_square(r as u8, f as u8).to_bitboard();
        }
    }

    attacks
}
