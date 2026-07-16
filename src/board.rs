use crate::types::{
    bitboard::Bitboard,
    castling::Castling,
    color::Color,
    hash_keys::HashKeys,
    piece::{Piece, PieceType},
    square::Square,
};

#[derive(Copy, Clone, Default)]
struct BoardState {
    castling: Castling,
    captured: Piece,
    en_passant: Square,
    half_move_clock: u8, // For the 50 move rule
    hash_keys: HashKeys,
}

#[derive(Clone)]
pub struct Board {
    piece_bitboards: [Bitboard; PieceType::NUM], // wow ... I was so confused on how this works - this is so nice
    color_bitboards: [Bitboard; Color::NUM],
    piece_squares: [Piece; Square::NUM],

    board_state: BoardState,
    board_state_stack: Vec<BoardState>,

    half_move_number: usize, // & 1 to get turn
}

impl Board {
    pub fn get_piece_on_square(&self, square: Square) -> Piece {
        self.piece_squares[square]
    }

    pub fn get_color(&self, color: Color) -> Bitboard {
        self.color_bitboards[color]
    }

    pub fn get_all_pieces(&self, pt: PieceType) -> Bitboard {
        self.piece_bitboards[pt]
    }

    pub fn get_piece(&self, piece: Piece) -> Bitboard {
        self.piece_bitboards[piece.piece_type()] & self.color_bitboards[piece.color()]
    }

    pub fn occupied(&self) -> Bitboard {
        self.color_bitboards[Color::White] | self.color_bitboards[Color::Black]
    }
}
