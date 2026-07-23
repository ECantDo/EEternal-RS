use std::{
    fmt::Display,
    ops::{Index, IndexMut},
};

use crate::types::color::Color;

// =================================================================================================
// Declare types
// =================================================================================================

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

// =================================================================================================
// Piece Impl
// =================================================================================================

impl Piece {
    pub const NUM: usize = 12;

    pub const ALL: [Self; Self::NUM] = [
        Self::WhitePawn,
        Self::BlackPawn,
        Self::WhiteKnight,
        Self::BlackKnight,
        Self::WhiteBishop,
        Self::BlackBishop,
        Self::WhiteRook,
        Self::BlackRook,
        Self::WhiteQueen,
        Self::BlackQueen,
        Self::WhiteKing,
        Self::BlackKing,
    ];

    pub const fn new(color: Color, piece_type: PieceType) -> Self {
        unsafe { std::mem::transmute(((piece_type as u8) << 1) | (color as u8)) }
    }

    pub const fn piece_type(self) -> PieceType {
        unsafe { std::mem::transmute(self as u8 >> 1) }
    }

    pub const fn color(self) -> Color {
        unsafe { std::mem::transmute(self as u8 & 1) }
    }

    // pub const fn value(self) -> i32 {
    //     self.piece_type().value()
    // }
    pub const fn value(self) -> i32 { // Todo: Swap with raw value, not side
        match self {
            Self::WhitePawn => 100,
            Self::WhiteKnight => 300,
            Self::WhiteBishop => 400,
            Self::WhiteRook => 600,
            Self::WhiteQueen => 1100,
            Self::WhiteKing => 0,
            Self::BlackPawn => -100,
            Self::BlackKnight => -300,
            Self::BlackBishop => -400,
            Self::BlackRook => -600,
            Self::BlackQueen => -1100,
            Self::BlackKing => 0,
            Self::None => 0,
        }
    }

    pub const fn from_index(i: usize) -> Self {
        debug_assert!(i < Self::NUM);
        unsafe { std::mem::transmute(i as u8) }
    }
}

impl TryFrom<char> for Piece {
    type Error = ();
    fn try_from(value: char) -> Result<Self, Self::Error> {
        let index = "PpNnBbRrQqKk".find(value).ok_or(())?;
        Ok(Self::from_index(index))
    }
}

impl TryInto<char> for Piece {
    type Error = ();

    fn try_into(self) -> Result<char, Self::Error> {
        let c = match self {
            Self::WhitePawn => 'P',
            Self::BlackPawn => 'p',
            Self::WhiteKnight => 'N',
            Self::BlackKnight => 'n',
            Self::WhiteBishop => 'B',
            Self::BlackBishop => 'b',
            Self::WhiteRook => 'R',
            Self::BlackRook => 'r',
            Self::WhiteQueen => 'Q',
            Self::BlackQueen => 'q',
            Self::WhiteKing => 'K',
            Self::BlackKing => 'k',
            Self::None => return Err(()),
        };
        Ok(c)
    }
}

impl Display for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", TryInto::<char>::try_into(*self).unwrap())
    }
}

impl<T> Index<Piece> for [T] {
    type Output = T;

    fn index(&self, index: Piece) -> &Self::Output {
        &self[index as usize]
    }
}

impl<T> IndexMut<Piece> for [T] {
    fn index_mut(&mut self, index: Piece) -> &mut Self::Output {
        &mut self[index as usize]
    }
}

// =================================================================================================
// PieceType Impl
// =================================================================================================
impl PieceType {
    pub const NUM: usize = 6;

    pub const ALL: [Self; Self::NUM] = [
        PieceType::Pawn,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Queen,
        PieceType::King,
    ];

    pub const fn new(value: usize) -> Self {
        debug_assert!(value < Self::NUM);
        unsafe { std::mem::transmute(value as u8) }
    }

    pub const fn value(self) -> i32 {
        match self {
            Self::Pawn => 100,
            Self::Knight => 300,
            Self::Bishop => 400,
            Self::Rook => 600,
            Self::Queen => 1100,
            Self::King => 0,
            Self::None => 0,
        }
    }
}

impl<T> Index<PieceType> for [T] {
    type Output = T;

    fn index(&self, index: PieceType) -> &Self::Output {
        &self[index as usize]
    }
}

impl<T> IndexMut<PieceType> for [T] {
    fn index_mut(&mut self, index: PieceType) -> &mut Self::Output {
        &mut self[index as usize]
    }
}
