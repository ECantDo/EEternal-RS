use std::ops::{Index, IndexMut};

use super::square::Square;
use crate::{board::Board, types::color::Color};

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum CastlingDirections {
    WhiteKingside = 0b0001,
    WhiteQueenside = 0b0010,
    BlackKingside = 0b0100,
    BlackQueenside = 0b1000,
}

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

    pub const fn rook_square(self) -> Square {
        match self {
            Self::WhiteKingside => Square::F1,
            Self::WhiteQueenside => Square::D1,
            Self::BlackKingside => Square::F8,
            Self::BlackQueenside => Square::D8,
        }
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

    pub fn to_string(self, board: &Board) -> String {
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