mod parse;

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

    pub fn pieces(&self, pt: PieceType) -> Bitboard {
        self.piece_bitboards[pt]
    }

    pub fn colored_pieces(&self, piece: Piece) -> Bitboard {
        self.piece_bitboards[piece.piece_type()] & self.color_bitboards[piece.color()]
    }

    pub fn occupancies(&self) -> Bitboard {
        self.color_bitboards[Color::White] | self.color_bitboards[Color::Black]
    }

    pub const fn full_move_number(&self) -> usize {
        self.half_move_number / 2
    }

    pub fn side_to_move(&self) -> Color {
        Color::new((self.half_move_number & 1) as u8)
    }

    pub fn en_passant(&self) -> Square {
        self.board_state.en_passant
    }

    pub fn hash(&self) -> u64 {
        self.board_state.hash_keys.zobrist()
    }

    pub fn add_piece(&mut self, piece: Piece, square: Square) {
        self.piece_squares[square] = piece;
        self.color_bitboards[piece.color()].set(square);
        self.piece_bitboards[piece.piece_type()].set(square);
        self.board_state.hash_keys.toggle(piece, square);
    }

    pub fn remove_piece(&mut self, square: Square) -> Piece {
        let piece = self.piece_squares[square];
        self.piece_squares[square] = Piece::None;
        self.color_bitboards[piece.color()].clear(square);
        self.piece_bitboards[piece.piece_type()].clear(square);
        self.board_state.hash_keys.toggle(piece, square);
        piece
    }

    /// Checks for a material draw
    pub fn draw_by_material(&self) -> bool {
        // I will help myself to some other peoples code for this too, I am lazy...
        let stm = self.side_to_move();
        if (self.pieces(PieceType::Pawn)
            | self.pieces(PieceType::Rook)
            | self.pieces(PieceType::Queen))
            != Bitboard(0)
        {
            return false;
        }

        let piece_count = self.occupancies().popcount();
        if piece_count != 4 {
            return piece_count < 4;
        }

        // Here on, there are exactly 2 non-king minors

        // Here, each side has one minor
        let bishop = Piece::new(stm, PieceType::Bishop);
        let knight = Piece::new(stm, PieceType::Knight);
        if (self.colored_pieces(bishop) | self.colored_pieces(knight)).popcount() == 1 {
            //If a king is in a corner, don't auto draw.
            return (Bitboard::CORNERS & self.pieces(PieceType::King)).is_empty();
        }

        if self.pieces(PieceType::Knight) != Bitboard(0) {
            return false;
        }

        (self.pieces(PieceType::Bishop) & Bitboard::LIGHT_SQUARES).popcount() != 1
    }

    pub fn draw_by_fifty_moves(&self) -> bool {
        self.half_move_number >= 100
    }

    pub fn is_draw(&self) -> bool {
        self.draw_by_fifty_moves() || self.draw_by_material() // TODO: Draw by repetition
    }

    pub fn refresh_hash(&mut self) {
        self.board_state.hash_keys = HashKeys::default();

        for piece in 0..Piece::NUM {
            let piece = Piece::from_index(piece);
            for square in self.colored_pieces(piece) {
                self.board_state.hash_keys.toggle(piece, square);
            }
        }

        if self.en_passant() != Square::None {
            self.board_state.hash_keys.toggle_en_passant(self.en_passant());
        }

        if self.side_to_move() == Color::White {
            self.board_state.hash_keys.toggle_side();
        }

        self.board_state.hash_keys.toggle_castling(self.board_state.castling);
    }
}

impl Default for Board {
    fn default() -> Self {
        Self {
            board_state: BoardState::default(),
            board_state_stack: Vec::with_capacity(2048),

            half_move_number: 0,

            piece_squares: [Piece::None; Square::NUM],
            color_bitboards: [Bitboard::default(); Color::NUM],
            piece_bitboards: [Bitboard::default(); PieceType::NUM],
        }
    }
}
