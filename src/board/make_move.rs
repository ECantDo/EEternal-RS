use crate::board::Board;
use crate::types::UP_DIR;
use crate::types::castling::{CASTLING_RIGHTS, castling_rook_squares};
use crate::types::moves::{Move, MoveFlag};
use crate::types::piece::{Piece, PieceType};
use crate::types::square::Square;

impl Board {
    pub fn make_move(&mut self, mv: Move) {
        let from = mv.from();
        let to = mv.to();
        let piece = self.get_piece_on_square(from);
        let stm = self.side_to_move();

        self.board_state_stack.push(self.board_state);

        // Clear old en passant square before anything else touches it
        if self.board_state.en_passant != Square::None {
            self.board_state
                .hash_keys
                .toggle_en_passant(self.board_state.en_passant);
            self.board_state.en_passant = Square::None;
        }

        // Fifty-move rule
        if piece.piece_type() == PieceType::Pawn || mv.is_capture() {
            self.board_state.half_move_clock = 0;
        } else {
            self.board_state.half_move_clock = self.board_state.half_move_clock.saturating_add(1);
        }

        self.board_state.captured = Piece::None;

        if mv.is_castling() {
            let (rook_from, rook_to) = castling_rook_squares(stm, mv.flag());
            self.remove_piece(from);
            self.add_piece(piece, to);
            let rook = self.remove_piece(rook_from);
            self.add_piece(rook, rook_to);
        } else if mv.is_en_passant() {
            self.remove_piece(from);
            self.add_piece(piece, to);
            let cap_sq = to.shift(-UP_DIR[stm]);
            self.board_state.captured = self.remove_piece(cap_sq);
        } else {
            if mv.is_capture() {
                self.board_state.captured = self.remove_piece(to);
            }
            self.remove_piece(from);

            if mv.is_promotion() {
                self.add_piece(Piece::new(stm, mv.promotion_piece_type()), to);
            } else {
                self.add_piece(piece, to);

                if mv.flag() == MoveFlag::DoublePush {
                    self.board_state.en_passant = Square::new(((from as u8) + (to as u8)) / 2);
                    self.board_state
                        .hash_keys
                        .toggle_en_passant(self.board_state.en_passant);
                }
            }
        }

        self.board_state
            .hash_keys
            .toggle_castling(self.board_state.castling);
        self.board_state.castling.raw &=
            CASTLING_RIGHTS[from as usize] & CASTLING_RIGHTS[to as usize];
        self.board_state
            .hash_keys
            .toggle_castling(self.board_state.castling);

        self.board_state.hash_keys.toggle_side();
        self.half_move_number += 1;
    }

    pub fn undo_move(&mut self, mv: Move) {
        self.half_move_number -= 1;
        let stm = self.side_to_move(); // color that made this move, now that we've stepped back

        let from = mv.from();
        let to = mv.to();

        if mv.is_castling() {
            let (rook_from, rook_to) = castling_rook_squares(stm, mv.flag());
            let king = self.remove_piece(to);
            let rook = self.remove_piece(rook_to);
            self.add_piece(king, from);
            self.add_piece(rook, rook_from);
        } else {
            let moved = self.remove_piece(to);
            let restored = if mv.is_promotion() { Piece::new(stm, PieceType::Pawn) } else { moved };
            self.add_piece(restored, from);

            if mv.is_en_passant() {
                let cap_sq = to.shift(-UP_DIR[stm]);
                self.add_piece(self.board_state.captured, cap_sq);
            } else if mv.is_capture() {
                self.add_piece(self.board_state.captured, to);
            }
        }

        self.board_state = self.board_state_stack.pop().unwrap();
    }
}
