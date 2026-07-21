use crate::types::castling::CastlingDirections;
use crate::types::moves::Move;
use crate::types::{PAWN_START, UP_DIR};
use crate::{
    attacking::{
        get_bishop_attacks, get_king_attacks, get_knight_attacks, get_pawn_attacks,
        get_queen_attacks, get_rook_attacks,
    },
    types::{
        bitboard::Bitboard,
        color::Color,
        move_list::MoveList,
        moves::MoveFlag,
        piece::PieceType,
        square::Square,
    },
};

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum MoveGenType {
    Quiet,
    Captures,
}

const PIN_DIRS: [i8; 8] = [8, -8, 1, -1, 9, 7, -7, -9];

impl super::Board {
    fn compute_pinned_pieces(&self, stm: Color, king_sq: Square) -> Bitboard {
        let mut pinned = Bitboard(0);
        let occ = self.occupancies();
        let own = self.get_color(stm);
        let enemy_rooks = self.colored_pieces(!stm, PieceType::Rook);
        let enemy_bishops = self.colored_pieces(!stm, PieceType::Bishop);
        let enemy_queens = self.colored_pieces(!stm, PieceType::Queen);

        for (d, &dir) in PIN_DIRS.iter().enumerate() {
            let mut potential_pin: Option<Square> = None;
            let mut sq: i8 = king_sq as i8 + dir;

            loop {
                if sq < 0 || sq >= 64 {
                    break;
                }

                let from_file = (sq - dir) % 8;
                let to_file = sq % 8;
                if (from_file - to_file).abs() > 1 {
                    break; // wrapped across the board edge
                }

                let square = Square::new(sq as u8);
                let sq_bb = square.to_bitboard();

                if (occ & sq_bb).not_empty() {
                    if (own & sq_bb).not_empty() {
                        match potential_pin {
                            None => potential_pin = Some(square),
                            Some(_) => break, // second friendly piece — no pin possible
                        }
                    } else {
                        let is_slider = if d < 4 {
                            (enemy_rooks & sq_bb).not_empty() || (enemy_queens & sq_bb).not_empty()
                        } else {
                            (enemy_bishops & sq_bb).not_empty()
                                || (enemy_queens & sq_bb).not_empty()
                        };

                        if is_slider {
                            if let Some(pin_sq) = potential_pin {
                                pinned.set(pin_sq);
                            }
                        }
                        break;
                    }
                }

                sq += dir;
            }
        }

        pinned
    }
}

impl super::Board {
    pub fn generate_all_legal_moves(&mut self, captures_only: bool) -> MoveList {
        let mut ml = self.generate_all_pseudolegal_moves(captures_only);

        let stm = self.side_to_move();
        let king_sq = self.king_square(stm);
        let we_are_in_check = self.is_square_attacked(king_sq, !stm);

        let pinned = if we_are_in_check {
            Bitboard(0)
        } else {
            self.compute_pinned_pieces(stm, king_sq)
        };

        let mut idx = 0;
        while idx < ml.len() {
            let mv = ml.get(idx).mv();
            let from = mv.from();
            let is_en_passant = mv.is_en_passant();

            let needs_verification =
                from == king_sq || is_en_passant || we_are_in_check || pinned.contains(from);

            if needs_verification {
                self.make_move(mv);
                let causes_check = self.is_square_attacked(self.king_square(stm), !stm);
                self.undo_move(mv);

                if causes_check {
                    ml.remove(idx); // swap-remove — don't advance idx, recheck what got swapped in
                } else {
                    idx += 1;
                }
            } else {
                idx += 1;
            }
        }

        ml
    }

    fn castling_passes_through_check(&self, mv: Move, stm: Color) -> bool {
        let king_from = mv.from();
        let king_to = mv.to();
        let step = if king_to as u8 > king_from as u8 {
            1
        } else {
            -1
        };
        let pass_through = king_from.shift(step); // the square between start and landing

        self.is_square_attacked(king_from, !stm)
            || self.is_square_attacked(pass_through, !stm)
            || self.is_square_attacked(king_to, !stm)
    }

    pub fn generate_all_pseudolegal_moves(&self, captures_only: bool) -> MoveList {
        let mut ml = MoveList::new();
        self.append_pawn_moves(&mut ml, captures_only); // from earlier
        self.append_knight_moves(&mut ml, captures_only);
        self.append_bishop_moves(&mut ml, captures_only);
        self.append_rook_moves(&mut ml, captures_only);
        self.append_queen_moves(&mut ml, captures_only);
        self.append_king_moves(&mut ml, captures_only);
        ml
    }

    pub fn append_rook_moves(&self, ml: &mut MoveList, captures_only: bool) {
        let stm = self.side_to_move();
        let own = self.get_color(stm);
        let enemy = self.get_color(!stm);
        let occ = self.occupancies();
        let rooks = self.colored_pieces(stm, PieceType::Rook);

        self.append_slider_or_knight_moves(ml, rooks, own, enemy, captures_only, |sq| {
            get_rook_attacks(sq, occ)
        });
    }

    pub fn append_bishop_moves(&self, ml: &mut MoveList, captures_only: bool) {
        let stm = self.side_to_move();
        let own = self.get_color(stm);
        let enemy = self.get_color(!stm);
        let occ = self.occupancies();
        let bishops = self.colored_pieces(stm, PieceType::Bishop);

        self.append_slider_or_knight_moves(ml, bishops, own, enemy, captures_only, |sq| {
            get_bishop_attacks(sq, occ)
        });
    }

    pub fn append_queen_moves(&self, ml: &mut MoveList, captures_only: bool) {
        let stm = self.side_to_move();
        let own = self.get_color(stm);
        let enemy = self.get_color(!stm);
        let occ = self.occupancies();
        let queens = self.colored_pieces(stm, PieceType::Queen);

        self.append_slider_or_knight_moves(ml, queens, own, enemy, captures_only, |sq| {
            get_queen_attacks(sq, occ)
        });
    }

    pub fn append_knight_moves(&self, ml: &mut MoveList, captures_only: bool) {
        let stm = self.side_to_move();
        let own = self.get_color(stm);
        let enemy = self.get_color(!stm);
        let knights = self.colored_pieces(stm, PieceType::Knight);

        self.append_slider_or_knight_moves(
            ml,
            knights,
            own,
            enemy,
            captures_only,
            get_knight_attacks,
        );
    }

    fn append_slider_or_knight_moves<F: Fn(Square) -> Bitboard>(
        &self,
        ml: &mut MoveList,
        pieces: Bitboard,
        own: Bitboard,
        enemy: Bitboard,
        captures_only: bool,
        attacks: F,
    ) {
        for from in pieces {
            let legal = attacks(from) & !own;

            for to in legal & enemy {
                ml.push(Move::new(from, to, MoveFlag::Capture));
            }

            if !captures_only {
                for to in legal & !enemy {
                    ml.push(Move::new(from, to, MoveFlag::Quiet));
                }
            }
        }
    }

    fn append_pawn_moves(&self, ml: &mut MoveList, captures_only: bool) {
        let stm = self.side_to_move();
        let up = UP_DIR[stm];
        let start_rank = PAWN_START[stm];

        let pawns = self.colored_pieces(stm, PieceType::Pawn);
        let empty = !self.occupancies();
        let evil_enemy = self.get_color(!stm);

        let ep = self.en_passant();

        for from in pawns {
            // Forward Push
            // There will never be a pawn (legally) on the back rank for the shift up to go out
            // of bounds
            if !captures_only {
                let one_forward = from.shift(up);

                if empty.contains(one_forward) {
                    // If moving onto the others home row ... promote
                    if Bitboard::BOTH_HOME_ROWS.contains(one_forward) {
                        ml.push(Move::new(from, one_forward, MoveFlag::PromoQueen));
                        ml.push(Move::new(from, one_forward, MoveFlag::PromoRook));
                        ml.push(Move::new(from, one_forward, MoveFlag::PromoBishop));
                        ml.push(Move::new(from, one_forward, MoveFlag::PromoKnight));
                    } else {
                        // Otherwise, just push
                        ml.push(Move::new(from, one_forward, MoveFlag::Quiet));

                        // And if on start rank; and one forward is clear; check 2 forward
                        if from.get_rank() == start_rank {
                            let two_forward = one_forward.shift(up);
                            if empty.contains(two_forward) {
                                ml.push(Move::new(from, two_forward, MoveFlag::DoublePush));
                            }
                        }
                    }
                }
            }

            // Captures
            let targets = get_pawn_attacks(from, stm);
            for to in targets & evil_enemy {
                if Bitboard::BOTH_HOME_ROWS.contains(to) {
                    ml.push(Move::new(from, to, MoveFlag::PromoCaptureQueen));
                    ml.push(Move::new(from, to, MoveFlag::PromoCaptureRook));
                    ml.push(Move::new(from, to, MoveFlag::PromoCaptureBishop));
                    ml.push(Move::new(from, to, MoveFlag::PromoCaptureKnight));
                } else {
                    ml.push(Move::new(from, to, MoveFlag::Capture));
                }
            }
        } // End of `for from ...`

        // Only if ep can happen, check for pawns that can
        if ep != Square::None {
            let ep_attackers = get_pawn_attacks(ep, !stm) & pawns;
            for from in ep_attackers {
                ml.push(Move::new(from, ep, MoveFlag::EnPassant));
            }
        }
    }
    pub fn append_king_moves(&self, ml: &mut MoveList, captures_only: bool) {
        let stm = self.side_to_move();
        let own = self.get_color(stm);
        let enemy = self.get_color(!stm);
        let king_sq = self.king_square(stm);

        let legal = get_king_attacks(king_sq) & !own;

        for to in legal & enemy {
            ml.push(Move::new(king_sq, to, MoveFlag::Capture));
        }

        if captures_only {
            return;
        }

        for to in legal & !enemy {
            ml.push(Move::new(king_sq, to, MoveFlag::Quiet));
        }

        // Can't castle out of check
        if self.is_square_attacked(king_sq, !stm) {
            return;
        }

        let occ = self.occupancies();

        match stm {
            Color::White => {
                if self
                    .board_state
                    .castling
                    .is_allowed(CastlingDirections::WhiteKingside)
                    && (Bitboard(0x60) & occ).is_empty()
                    && !self.is_square_attacked(Square::F1, Color::Black)
                {
                    ml.push(Move::new(Square::E1, Square::G1, MoveFlag::CastleKingside));
                }
                if self
                    .board_state
                    .castling
                    .is_allowed(CastlingDirections::WhiteQueenside)
                    && (Bitboard(0x0E) & occ).is_empty()
                    && !self.is_square_attacked(Square::D1, Color::Black)
                {
                    ml.push(Move::new(Square::E1, Square::C1, MoveFlag::CastleQueenside));
                }
            }
            Color::Black => {
                if self
                    .board_state
                    .castling
                    .is_allowed(CastlingDirections::BlackKingside)
                    && (Bitboard(0x6000000000000000) & occ).is_empty()
                    && !self.is_square_attacked(Square::F8, Color::White)
                {
                    ml.push(Move::new(Square::E8, Square::G8, MoveFlag::CastleKingside));
                }
                if self
                    .board_state
                    .castling
                    .is_allowed(CastlingDirections::BlackQueenside)
                    && (Bitboard(0x0E00000000000000) & occ).is_empty()
                    && !self.is_square_attacked(Square::D8, Color::White)
                {
                    ml.push(Move::new(Square::E8, Square::C8, MoveFlag::CastleQueenside));
                }
            }
        }
    }
}
