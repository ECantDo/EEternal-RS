use std::ops::{BitOrAssign, Index, IndexMut};

use super::square::Square;
use crate::{board::Board, types::color::Color};
use crate::types::moves::MoveFlag;

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum CastlingDirections {
    WhiteKingside = 0b0001,
    WhiteQueenside = 0b0010,
    BlackKingside = 0b0100,
    BlackQueenside = 0b1000,
}

pub const CASTLING_RIGHTS: [u8; 64] = {
    let mut table = [0b1111u8; 64];
    table[Square::E1 as usize] = 0b1100; // king moves: lose both white rights
    table[Square::A1 as usize] = 0b1101; // rook moves/captured: lose white queenside
    table[Square::H1 as usize] = 0b1110; // lose white kingside
    table[Square::E8 as usize] = 0b0011; // See above, but for black
    table[Square::A8 as usize] = 0b0111;
    table[Square::H8 as usize] = 0b1011;
    table
};

impl CastlingDirections {
    pub const DIRECTIONS: [[Self; 2]; 2] = [
        [Self::WhiteKingside, Self::WhiteQueenside],
        [Self::BlackKingside, Self::BlackQueenside],
    ];

    pub const fn king_square(self) -> Square {
        match self {
            Self::WhiteKingside => Square::G1,
            Self::WhiteQueenside => Square::C1,
            Self::BlackKingside => Square::G8,
            Self::BlackQueenside => Square::C8,
        }
    }
}

pub const fn castling_rook_squares(color: Color, flag: MoveFlag) -> (Square, Square) {
    match (color, flag) {
        (Color::White, MoveFlag::CastleKingside) => (Square::H1, Square::F1),
        (Color::White, MoveFlag::CastleQueenside) => (Square::A1, Square::D1),
        (Color::Black, MoveFlag::CastleKingside) => (Square::H8, Square::F8),
        (Color::Black, MoveFlag::CastleQueenside) => (Square::A8, Square::D8),
        _ => unreachable!(),
    }
}

impl<T> Index<CastlingDirections> for [T] {
    type Output = T;

    fn index(&self, index: CastlingDirections) -> &Self::Output {
        &self[index as usize]
    }
}

impl<T> IndexMut<CastlingDirections> for [T] {
    fn index_mut(&mut self, index: CastlingDirections) -> &mut Self::Output {
        &mut self[index as usize]
    }
}

#[derive(Copy, Clone, Default)]
pub struct Castling {
    pub raw: u8,
}

impl Castling {
    pub const fn raw(self) -> u8 {
        self.raw
    }

    pub const fn is_allowed(self, kind: CastlingDirections) -> bool {
        (self.raw & kind as u8) != 0
    }

    pub fn to_string(self, _board: &Board) -> String {
        if self.raw == 0 {
            return "-".to_string();
        }

        let mut result = String::new();

        let kinds = [
            (CastlingDirections::WhiteKingside, 'K', Color::White),
            (CastlingDirections::WhiteQueenside, 'Q', Color::White),
            (CastlingDirections::BlackKingside, 'k', Color::Black),
            (CastlingDirections::BlackQueenside, 'q', Color::Black),
        ];

        for (kind, symbol, _color) in kinds {
            if !self.is_allowed(kind) {
                continue;
            }
            result.push(symbol);
        }

        result
    }
}

impl<T> Index<Castling> for [T] {
    type Output = T;

    fn index(&self, index: Castling) -> &Self::Output {
        &self[index.raw as usize]
    }
}

impl BitOrAssign<CastlingDirections> for Castling {
    fn bitor_assign(&mut self, rhs: CastlingDirections) {
        self.raw |= rhs as u8
    }
}

impl TryFrom<&str> for Castling {
    type Error = String;
    fn try_from(value: &str) -> Result<Castling, Self::Error> {
        let mut castling: Castling = Castling { raw: 0 };
        for chr in value.chars() {
            match chr {
                'Q' => castling |= CastlingDirections::WhiteQueenside,
                'K' => castling |= CastlingDirections::WhiteKingside,
                'q' => castling |= CastlingDirections::BlackQueenside,
                'k' => castling |= CastlingDirections::BlackKingside,
                '-' => (),
                _ => {
                    return Err(format!("Invalid castling rules {chr}"));
                }
            }
        }
        Ok(castling)
    }
}
