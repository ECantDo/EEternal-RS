use crate::{
    attacking::{
        get_bishop_attacks, get_king_attacks, get_knight_attacks, get_pawn_attacks,
        get_queen_attacks, get_rook_attacks,
    },
    board::Board,
    types::{
        bitboard::Bitboard,
        castling::{castling_rook_squares, CASTLING_RIGHTS},
        color::Color,
        moves::{Move, MoveFlag},
        piece::{Piece, PieceType},
        square::Square,
        UP_DIR,
    },
};
use crate::rays::between;

impl Board {
    pub fn make_move(&mut self, mv: Move) {
        let from = mv.from();
        let to = mv.to();
        let my_piece = self.get_piece_on_square(from);
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
        if my_piece.piece_type() == PieceType::Pawn || mv.is_capture() {
            self.board_state.half_move_clock = 0;
        } else {
            self.board_state.half_move_clock = self.board_state.half_move_clock.saturating_add(1);
        }

        self.board_state.captured = Piece::None;

        if mv.is_castling() {
            let (rook_from, rook_to) = castling_rook_squares(stm, mv.flag());
            self.remove_piece(from);
            self.add_piece(my_piece, to);
            let rook = self.remove_piece(rook_from);
            self.add_piece(rook, rook_to);
        } else if mv.is_en_passant() {
            self.remove_piece(from);
            self.add_piece(my_piece, to);
            let cap_sq = to.shift(-UP_DIR[stm]);
            self.board_state.captured = self.remove_piece(cap_sq);
        } else {
            if mv.is_capture() {
                self.board_state.captured = self.remove_piece(to);
            }
            self.remove_piece(from);

            if mv.is_promotion() {
                let new_piece = Piece::new(stm, mv.promotion_piece_type());
                self.add_piece(new_piece, to);
                self.board_state.material +=
                    new_piece.value() - Piece::new(stm, PieceType::Pawn).value();
            } else {
                self.add_piece(my_piece, to);

                if mv.flag() == MoveFlag::DoublePush {
                    self.board_state.en_passant = Square::new(((from as u8) + (to as u8)) / 2);
                    self.board_state
                        .hash_keys
                        .toggle_en_passant(self.board_state.en_passant);
                }
            }
        }

        self.board_state.material -= self.board_state.captured.value();

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

        // Refresh threats when switching moves.
        self.refresh_piece_threats();

        #[cfg(debug_assertions)]
        self.assert_material_consistent();
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
            let restored = if mv.is_promotion() {
                Piece::new(stm, PieceType::Pawn)
            } else {
                moved
            };
            self.add_piece(restored, from);

            if mv.is_en_passant() {
                let cap_sq = to.shift(UP_DIR[!stm]);
                self.add_piece(self.board_state.captured, cap_sq);
            } else if mv.is_capture() {
                self.add_piece(self.board_state.captured, to);
            }
        }

        // The material doesn't need to be changed, it is saved on the stack

        self.board_state = self.board_state_stack.pop().unwrap();

        #[cfg(debug_assertions)]
        self.assert_material_consistent();
    }

    #[cfg(debug_assertions)]
    fn assert_material_consistent(&self) {
        let mut recomputed = 0;
        for pt in [
            PieceType::Pawn,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Rook,
            PieceType::Queen,
        ] {
            recomputed +=
                self.colored_pieces(Color::White, pt).popcount() as i32 * PieceType::value(pt);
            recomputed +=
                self.colored_pieces(Color::Black, pt).popcount() as i32 * PieceType::value(pt);
        }
        debug_assert_eq!(
            recomputed * 100 / 100,
            self.board_state.material,
            "material cache desynced"
        );
    }

    pub fn refresh_piece_threats(&mut self) {
        let stm = self.side_to_move();
        let occ = self.occupancies();
        self.board_state.threats_by = [Bitboard(0); Color::NUM];


        for color in Color::ALL {
            let occ_missing_their_king = occ ^ self.colored_pieces(!color, PieceType::King);
            for piece_type in PieceType::ALL {
                let get_attack: fn(Square, Bitboard) -> Bitboard = match piece_type {
                    PieceType::Pawn => {
                        if color == Color::White {
                            |sq, _occ| get_pawn_attacks(sq, Color::White)
                        } else {
                            |sq, _occ| get_pawn_attacks(sq, Color::Black)
                        }
                    }
                    PieceType::Knight => |sq, _occ| get_knight_attacks(sq),
                    PieceType::King => |sq, _occ| get_king_attacks(sq),
                    PieceType::Bishop => get_bishop_attacks,
                    PieceType::Rook => get_rook_attacks,
                    PieceType::Queen => get_queen_attacks,
                    PieceType::None => unreachable!(),
                };
                let mut attacked = Bitboard(0);

                for sq in self.colored_pieces(color, piece_type) {
                    debug_assert!(sq != Square::None);
                    attacked |= get_attack(sq, occ_missing_their_king);
                }


                self.board_state.threats_by[color] |= attacked;
                self.board_state.piece_threats[color][piece_type] = attacked;
            }
        }

        self.update_check_info();
    }

    pub fn update_check_info(&mut self) {
        let stm = self.side_to_move();

        self.board_state.checkers = Bitboard(0);
        self.board_state.pinned = [Bitboard(0); Color::NUM];
        self.board_state.pinners = [Bitboard(0); Color::NUM];

        for color in [Color::White, Color::Black] {
            let king = self.king_square(color);
            let enemy = !color;

            // "See through" own-color pieces: attacks computed with only enemy
            // pieces as occupancy find every slider that *could* be pinning or
            // checking, before we know how many of our own pieces sit in the way.
            let diagonal = get_bishop_attacks(king, self.get_color(enemy))
                & (self.colored_pieces(enemy, PieceType::Bishop) | self.colored_pieces(enemy, PieceType::Queen));
            let orthogonal = get_rook_attacks(king, self.get_color(enemy))
                & (self.colored_pieces(enemy, PieceType::Rook) | self.colored_pieces(enemy, PieceType::Queen));

            for candidate in diagonal | orthogonal {
                let blockers = between(king, candidate) & self.get_color(color);
                match blockers.popcount() {
                    0 => {
                        if color == stm {
                            self.board_state.checkers.set(candidate);
                        }
                    }
                    1 => {
                        self.board_state.pinned[color] |= blockers;
                        self.board_state.pinners[enemy].set(candidate);
                    }
                    _ => (),
                }
            }
        }

        // Non-slider checks against the side to move.
        let king = self.king_square(stm);
        self.board_state.checkers |=
            (get_pawn_attacks(king, stm) & self.colored_pieces(!stm, PieceType::Pawn))
                | (get_knight_attacks(king) & self.colored_pieces(!stm, PieceType::Knight));

        // Exclude our own king from occupancy so sliding attacks correctly
        // project through the square it's about to vacate.
        // let occ_without_king = self.occupancies() ^ self.colored_pieces(stm, PieceType::King);
        // self.board_state.all_threats = self.attacked_squares(!stm, occ_without_king);
    }
}
