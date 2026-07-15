use std::{
    fmt::Display,
    ops::{Index, IndexMut},
};

use crate::types::color::Color;

#[derive(Copy, Clone, Default, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum Piece {
    WhitePawn,
    BlackPawn,
    WhiteKnight,
    BlackKnight,
    WhiteBishop,
    BlackBishop,
    WhiteRook,
    BlackRook,
    WhiteQueen,
    BlackQueen,
    WhiteKing,
    BlackKing,
    #[default]
    None,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
    None,
}

impl Piece {
    pub const NUM: usize = 12;

    pub const ALL: [Self; Self::NUM] = [
        Self::WhitePawn,
        Self::BlackPawn,
        Self::WhiteBishop,
        Self::BlackBishop,
        Self::WhiteKnight,
        Self::BlackKnight,
        Self::WhiteRook,
        Self::BlackRook,
        Self::WhiteQueen,
        Self::BlackQueen,
        Self::WhiteKing,
        Self::BlackKing,
    ];

    pub const fn piece_type(self) -> PieceType {
        unsafe { std::mem::transmute(self as u8 >> 1) }
    }

    pub const fn color(self) -> Color {
        unsafe { std::mem::transmute(self as u8 & 1) }
    }
}
