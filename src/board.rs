mod evaluate;
pub mod generate_moves;
mod make_move;
pub mod parse;
mod is_legal;

use crate::{
    attacking::{
        get_bishop_attacks, get_king_attacks, get_knight_attacks, get_pawn_attacks,
        get_rook_attacks,
    },
    types::{
        bitboard::Bitboard,
        castling::Castling,
        color::Color,
        hash_keys::HashKeys,
        piece::{Piece, PieceType},
        square::Square,
    },
};

#[derive(Copy, Clone, Default)]
struct BoardState {
    castling: Castling,
    captured: Piece,
    en_passant: Square,
    half_move_clock: u8, // For the 50 move rule
    hash_keys: HashKeys,
    material: i32,

    checkers: Bitboard,
    pinned: [Bitboard; Color::NUM],
    pinners: [Bitboard; Color::NUM],
    piece_threats: [[Bitboard; PieceType::NUM]; Color::NUM],
    threats_by: [Bitboard; Color::NUM],
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

    pub fn king_square(&self, color: Color) -> Square {
        debug_assert!(self.colored_pieces(color, PieceType::King).not_empty());
        self.colored_pieces(color, PieceType::King).lsb()
    }

    pub fn in_check(&self) -> bool {
        let stm = self.side_to_move();
        self.is_square_attacked(self.king_square(stm), !stm)
    }

    pub fn pieces(&self, pt: PieceType) -> Bitboard {
        self.piece_bitboards[pt]
    }

    pub fn colored_pieces(&self, color: Color, piece_type: PieceType) -> Bitboard {
        self.piece_bitboards[piece_type] & self.color_bitboards[color]
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

    pub const fn material(&self) -> i32 {
        self.board_state.material
    }

    pub fn add_piece(&mut self, piece: Piece, square: Square) {
        self.piece_squares[square] = piece;
        self.color_bitboards[piece.color()].set(square);
        self.piece_bitboards[piece.piece_type()].set(square);
        self.board_state.hash_keys.toggle(piece, square);
    }

    pub fn remove_piece(&mut self, square: Square) -> Piece {
        let piece = self.piece_squares[square];
        debug_assert!(piece != Piece::None);
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
        if (self.colored_pieces(stm, PieceType::Bishop)
            | self.colored_pieces(stm, PieceType::Knight))
        .popcount()
            == 1
        {
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

    pub fn draw_by_repetition(&self) -> bool {
        let len = self.board_state_stack.len();
        if len == 0 {
            return false;
        }
        let current: u64 = self.board_state.hash_keys.zobrist();
        let mut count: i32 = 1;
        let lookback =
            (self.board_state.half_move_clock as usize).min(len - 1);

        let mut i = 1;
        while i <= lookback {
            let state = self.board_state_stack[len - 1 - i];
            if state.hash_keys.zobrist() == current {
                // return true;
            }
            i += 2;
        }
        false
    }

    pub fn is_draw(&self) -> bool {
        self.draw_by_fifty_moves() || self.draw_by_material() || self.draw_by_repetition()
    }

    pub fn refresh_hash(&mut self) {
        self.board_state.hash_keys = HashKeys::default();

        for piece in 0..Piece::NUM {
            let piece = Piece::from_index(piece);
            for square in self.colored_pieces(piece.color(), piece.piece_type()) {
                self.board_state.hash_keys.toggle(piece, square);
            }
        }

        if self.en_passant() != Square::None {
            self.board_state
                .hash_keys
                .toggle_en_passant(self.en_passant());
        }

        if self.side_to_move() == Color::White {
            self.board_state.hash_keys.toggle_side();
        }

        self.board_state
            .hash_keys
            .toggle_castling(self.board_state.castling);
    }

    pub fn piece_type_attacks(&self, color: Color, piece_type: PieceType) -> Bitboard{
        debug_assert!(piece_type != PieceType::None);
        self.board_state.piece_threats[color][piece_type]
    }

    pub fn is_square_attacked(&self, square: Square, by: Color) -> bool {
        (self.board_state.threats_by[by] & square.to_bitboard()).not_empty()
    }

    pub fn checkers(&self) -> Bitboard {
        self.board_state.checkers
    }

    pub fn pinned(&self, color: Color) -> Bitboard {
        self.board_state.pinned[color]
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

impl std::fmt::Display for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "+---+---+---+---+---+---+---+---+")?;
        for rank in (0..8).rev() {
            write!(f, "|")?;
            for file in 0..8 {
                let square = Square::get_board_square(rank, file);
                let symbol: char = self.get_piece_on_square(square).try_into().unwrap_or(' ');
                write!(f, " {symbol} |")?;
            }
            writeln!(f, " {}", rank + 1)?;
            writeln!(f, "+---+---+---+---+---+---+---+---+")?;
        }
        writeln!(f, "  a   b   c   d   e   f   g   h")
    }
}
