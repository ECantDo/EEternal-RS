use super::Board;
use crate::types::castling::Castling;
use crate::types::color::Color;
use crate::types::piece::Piece;
use crate::types::square::Square;

pub const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

impl Board {
    pub fn parse_fen_string(fen: &str) -> Result<Self, String> {
        /*
        1. Piece placement
        2. Active color
        3. Castling
        4. En passant
        5. Half move clock
        6. Full move counter
         */

        let mut board = Self::default();
        let mut fen_parts = fen.split_whitespace();

        // Piece Placement
        let rows = fen_parts
            .next()
            .ok_or("Missing piece placement")?
            .split('/');

        for (rank, row) in rows.rev().enumerate() {
            let mut file: u8 = 0;

            for chr in row.chars() {
                if let Some(skip) = chr.to_digit(10) {
                    file += skip as u8;
                    continue;
                }

                let piece = Piece::try_from(chr).map_err(|_| format!("Bad piece {chr}"))?;
                let square = Square::get_board_square(rank as u8, file);

                board.add_piece(piece, square);
                board.board_state.material += piece.value();
                file += 1;
            }
        }

        // Turn
        let side_to_move = match fen_parts.next() {
            Some("w") => Color::White,
            Some("b") => Color::Black,
            _ => return Err("Bad side to move".to_string()),
        };

        // Castling
        let castling_str = fen_parts.next().ok_or("Missing castling")?;
        board.board_state.castling = Castling::try_from(castling_str)?;

        // En passant
        board.board_state.en_passant = fen_parts
            .next()
            .unwrap_or_default()
            .try_into()
            .unwrap_or_default();

        // 50 moves
        board.board_state.half_move_clock = fen_parts
            .next()
            .unwrap_or_default()
            .parse()
            .unwrap_or_default();

        // Full move
        let full_move: usize = fen_parts
            .next()
            .unwrap_or_default()
            .parse()
            .unwrap_or_default();
        board.half_move_number = full_move * 2 + (side_to_move as usize);

        board.refresh_hash();
        
        board.refresh_piece_threats();
        
        Ok(board)
    }

    pub fn startpos() -> Self {
        Self::parse_fen_string(STARTPOS).unwrap()
    }
}
