use crate::types::{castling::Castling, piece::Piece, square::Square, zobrist::ZOBRIST};

#[derive(Clone, Copy, Default)]
pub struct HashKeys {
    pub zob: u64,
}

impl HashKeys {
    pub const fn zobrist(&self) -> u64 {
        self.zob
    }

    pub fn toggle(&mut self, piece: Piece, sq: Square) {
        let pk = ZOBRIST.pieces[piece][sq];
        self.zob ^= pk;
    }

    pub fn toggle_side(&mut self) {
        self.zob ^= ZOBRIST.side;
    }

    pub fn toggle_castling(&mut self, castling: Castling) {
        self.zob ^= ZOBRIST.castling[castling];
    }

    pub fn toggle_en_passant(&mut self, en_passant: Square) {
        self.zob ^= ZOBRIST.en_passant[en_passant];
    }
}
