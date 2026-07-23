use crate::rays::between;
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
        self.update_move_threat(mv);
        // self.refresh_piece_threats();
        #[cfg(debug_assertions)]
        {
            let mut shadow = self.clone();
            shadow.refresh_piece_threats();
            debug_assert_eq!(
                shadow.board_state.piece_threats, self.board_state.piece_threats,
                "threats desync after {mv:?}"
            );
            debug_assert_eq!(shadow.board_state.threats_by, self.board_state.threats_by);
            debug_assert_eq!(shadow.board_state.checkers, self.board_state.checkers);
            debug_assert_eq!(shadow.board_state.pinned, self.board_state.pinned);
        }
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
    }

    fn recompute_bucket(&mut self, color: Color, piece_type: PieceType, occ: Bitboard) {
        let attack_fn = attack_fn_for(color, piece_type);
        let mut attacked = Bitboard(0);
        for sq in self.colored_pieces(color, piece_type) {
            attacked |= attack_fn(sq, occ);
        }
        self.board_state.piece_threats[color][piece_type] = attacked;
    }

    pub fn update_move_threat(&mut self, mv: Move) {
        debug_assert!(mv != Move::NONE);
        let occ = self.occupancies();
        let king_occ: [Bitboard; Color::NUM] = [
            occ ^ self.colored_pieces(Color::Black, PieceType::King),
            occ ^ self.colored_pieces(Color::White, PieceType::King),
        ];

        let mut affected_squares: [Square; 4] = [mv.from(), mv.to(), Square::None, Square::None];

        let mut updated: [[bool; PieceType::NUM]; Color::NUM] = Default::default();
        // Recompute bucket function basically (after checking if it hasn't been already updated).
        let mut mark = |this: &mut Self, color: Color, pt: PieceType| {
            if !updated[color][pt] {
                updated[color][pt] = true;
                this.recompute_bucket(color, pt, king_occ[color]);
            }
        };

        // Update the moved piece
        let moved_piece = self.get_piece_on_square(mv.to());
        mark(self, moved_piece.color(), moved_piece.piece_type());

        // Update rooks if castling
        if mv.is_castling() {
            mark(self, moved_piece.color(), PieceType::Rook);
            let rook_squares = castling_rook_squares(moved_piece.color(), mv.flag());
            affected_squares[2] = rook_squares.0;
            affected_squares[3] = rook_squares.1;
        } else {
            // If castled, we know that it's not a pawn move (i.e. no en passant nor promotion)
            // and we know that we are not capturing (castling can't capture). BUT the
            // aforementioned are not mutually exclusive (we can capture and promote)
            // =====================================================================================
            // Update the captured pieces ; should also handle en_passant, since those flags overlap
            // and the captured piece would be a pawn. Still need to add the EP square to affected
            // squares, but the captured piece update should be fine.
            if mv.is_capture() {
                mark(
                    self,
                    self.board_state.captured.color(),
                    self.board_state.captured.piece_type(),
                );
            }

            if mv.is_en_passant() {
                affected_squares[2] = mv.to().shift(UP_DIR[!moved_piece.color()]);
            }

            // Update pawns if promotion; the `moved_piece` will be the promoted piece, so just need
            // to do the pawns
            if mv.is_promotion() {
                mark(self, moved_piece.color(), PieceType::Pawn);
            }
        }

        // If the square is attacked, find the sliders that attack it (and update);
        // pawns, knights, and kings don't need updating, they will still attack the same squares.
        // Do it for both the `from` and `to` squares from the move.
        for sq in affected_squares {
            // Sine we "append" to the squares, once we hit a None, we can stop
            if sq == Square::None {
                break;
            }
            let rook_ray = get_rook_attacks(sq, occ);
            let bishop_ray = get_bishop_attacks(sq, occ);
            let mut hits = (rook_ray & (self.pieces(PieceType::Rook) | self.pieces(PieceType::Queen)))
                | (bishop_ray & (self.pieces(PieceType::Bishop) | self.pieces(PieceType::Queen)));

            while hits.not_empty() {
                let hit_sq = hits.lsb();
                let piece = self.get_piece_on_square(hit_sq);
                hits &= !self.colored_pieces(piece.color(), piece.piece_type());
                mark(self, piece.color(), piece.piece_type());
            }
            // for hit_sq in hits {
            //     let piece = self.get_piece_on_square(hit_sq);
            //     mark(self, piece.color(), piece.piece_type());
            // }
        }

        // Update the master threats
        for color in Color::ALL {
            self.board_state.threats_by[color] =
                PieceType::ALL.iter().fold(Bitboard(0), |acc, &pt| {
                    acc | self.board_state.piece_threats[color][pt]
                });
        }
        self.update_check_info();
    }

    pub fn refresh_piece_threats(&mut self) {
        let occ = self.occupancies();
        self.board_state.threats_by = [Bitboard(0); Color::NUM];

        for color in Color::ALL {
            let occ_missing_their_king = occ ^ self.colored_pieces(!color, PieceType::King);
            for piece_type in PieceType::ALL {
                let get_attack: fn(Square, Bitboard) -> Bitboard = attack_fn_for(color, piece_type);
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
                & (self.colored_pieces(enemy, PieceType::Bishop)
                    | self.colored_pieces(enemy, PieceType::Queen));
            let orthogonal = get_rook_attacks(king, self.get_color(enemy))
                & (self.colored_pieces(enemy, PieceType::Rook)
                    | self.colored_pieces(enemy, PieceType::Queen));

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
        self.board_state.checkers |= (get_pawn_attacks(king, stm)
            & self.colored_pieces(!stm, PieceType::Pawn))
            | (get_knight_attacks(king) & self.colored_pieces(!stm, PieceType::Knight));

        // Exclude our own king from occupancy so sliding attacks correctly
        // project through the square it's about to vacate.
        // let occ_without_king = self.occupancies() ^ self.colored_pieces(stm, PieceType::King);
        // self.board_state.all_threats = self.attacked_squares(!stm, occ_without_king);
    }

    #[cfg(debug_assertions)]
    fn assert_threats_consistent(&self) {
        let mut shadow = self.clone();
        shadow.refresh_piece_threats(); // your old, known-correct full loop
        debug_assert_eq!(
            shadow.board_state.piece_threats,
            self.board_state.piece_threats
        );
        debug_assert_eq!(shadow.board_state.threats_by, self.board_state.threats_by);
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
}

fn attack_fn_for(color: Color, piece_type: PieceType) -> (fn(Square, Bitboard) -> Bitboard) {
    match piece_type {
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
    }
}
