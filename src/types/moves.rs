use super::{piece::PieceType, square::Square};
use crate::board::Board;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Move(u16);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum MoveFlag {
    Quiet = 0,
    DoublePush = 1,
    CastleKingside = 2,
    CastleQueenside = 3,

    Capture = 4,
    EnPassant = 5,
    // promotions: bit pattern 8..=11, with capture bit (bit 2) OR'd in for promo-captures
    PromoKnight = 8,
    PromoBishop = 9,
    PromoRook = 10,
    PromoQueen = 11,
    PromoCaptureKnight = 12,
    PromoCaptureBishop = 13,
    PromoCaptureRook = 14,
    PromoCaptureQueen = 15,
}

impl Move {
    pub const NONE: Self = Self(0);

    pub const fn new(from: Square, to: Square, flag: MoveFlag) -> Self {
        Self((from as u16) | ((to as u16) << 6) | ((flag as u16) << 12))
    }

    pub const fn from(self) -> Square {
        Square::new((self.0 & 0x3F) as u8)
    }

    pub const fn to(self) -> Square {
        Square::new(((self.0 >> 6) & 0x3F) as u8)
    }

    pub const fn flag(self) -> MoveFlag {
        unsafe { std::mem::transmute(((self.0 >> 12) & 0xF) as u8) }
    }

    pub const fn is_none(self) -> bool {
        self.0 == 0
    }

    pub const fn is_capture(self) -> bool {
        (self.0 >> 12) & 0b0100 != 0 // bit 2 of the flag nibble
    }

    pub const fn is_promotion(self) -> bool {
        (self.0 >> 12) & 0b1000 != 0 // bit 3 of the flag nibble
    }

    pub const fn is_castling(self) -> bool {
        matches!(
            self.flag(),
            MoveFlag::CastleKingside | MoveFlag::CastleQueenside
        )
    }

    pub const fn is_en_passant(self) -> bool {
        matches!(self.flag(), MoveFlag::EnPassant)
    }

    pub const fn promotion_piece_type(self) -> PieceType {
        debug_assert!(self.is_promotion());
        PieceType::new(((self.flag() as usize) & 3) + PieceType::Knight as usize)
    }

    pub fn to_uci(self, board: &Board) -> String {
        let mut output = format!("{}{}", self.from(), self.to());

        if self.is_promotion() {
            match self.promotion_piece_type() {
                PieceType::Knight => output.push('n'),
                PieceType::Bishop => output.push('b'),
                PieceType::Rook => output.push('r'),
                PieceType::Queen => output.push('q'),
                _ => (),
            }
        }

        output
    }
}

impl Default for Move {
    fn default() -> Self {
        Move::NONE
    }
}
