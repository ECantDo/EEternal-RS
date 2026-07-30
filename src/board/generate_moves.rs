use crate::rays::{between, line_through};
use crate::types::castling::CastlingDirections;
use crate::types::moves::{Move, MoveFlag};
use crate::types::{PAWN_START, UP_DIR};
use crate::{
    attacking::{
        get_bishop_attacks, get_king_attacks, get_knight_attacks, get_pawn_attacks,
        get_queen_attacks, get_rook_attacks,
    },
    types::{
        bitboard::Bitboard, color::Color, move_list::MoveList, piece::PieceType, square::Square,
    },
};

#[derive(Copy, Clone, Eq, PartialEq)]
enum MovegenKind {
    Quiet,
    Noisy,
}

// move_ordering.rs (or wherever ScoreType/NodeType-style traits live)
pub trait GenType {
    const CAPTURES_ONLY: bool;
}

pub struct AllMoves;
impl GenType for AllMoves {
    const CAPTURES_ONLY: bool = false;
}

pub struct CapturesOnly;
impl GenType for CapturesOnly {
    const CAPTURES_ONLY: bool = true;
}

impl super::Board {
    pub fn generate_all_legal_moves<G: GenType>(&self) -> MoveList {
        let mut ml = MoveList::new();
        let stm = self.side_to_move();
        let occ = self.occupancies();
        let own = self.get_color(stm);
        let enemy = self.get_color(!stm);
        let enemy_threats = self.board_state.threats_by[!stm];

        let king_sq = self.king_square(stm);
        let king_targets = if G::CAPTURES_ONLY { enemy } else { !own };
        for to in get_king_attacks(king_sq) & !own & !enemy_threats & king_targets {
            let flag = if enemy.contains(to) {
                MoveFlag::Capture
            } else {
                MoveFlag::Quiet
            };
            ml.push(Move::new(king_sq, to, flag));
        }

        if self.checkers().is_multiple() {
            return ml;
        }

        let mut target = if self.checkers().not_empty() {
            (between(king_sq, self.checkers().lsb()) | self.checkers()) & !own
        } else {
            !own
        };
        if G::CAPTURES_ONLY {
            target &= enemy;
        }

        let pinned = self.pinned(stm);

        self.collect_pawn_moves::<G>(&mut ml, target, pinned);

        for knight in self.colored_pieces(stm, PieceType::Knight) & !pinned {
            for to in get_knight_attacks(knight) & target {
                let flag = if enemy.contains(to) {
                    MoveFlag::Capture
                } else {
                    MoveFlag::Quiet
                };
                ml.push(Move::new(knight, to, flag));
            }
        }

        let bishops = self.colored_pieces(stm, PieceType::Bishop);
        let rooks = self.colored_pieces(stm, PieceType::Rook);
        let queens = self.colored_pieces(stm, PieceType::Queen);

        self.collect_slider(&mut ml, target, bishops, pinned, king_sq, |sq| {
            get_bishop_attacks(sq, occ)
        });
        self.collect_slider(&mut ml, target, rooks, pinned, king_sq, |sq| {
            get_rook_attacks(sq, occ)
        });
        self.collect_slider(&mut ml, target, queens, pinned, king_sq, |sq| {
            get_queen_attacks(sq, occ)
        });

        if !G::CAPTURES_ONLY {
            self.collect_castling(&mut ml);
        }

        ml
    }

    fn collect_slider<F: Fn(Square) -> Bitboard>(
        &self,
        ml: &mut MoveList,
        target: Bitboard,
        pieces: Bitboard,
        pinned: Bitboard,
        king_sq: Square,
        attacks: F,
    ) {
        let enemy = self.get_color(!self.side_to_move());

        for from in pieces & !pinned {
            for to in attacks(from) & target {
                let flag = if enemy.contains(to) {
                    MoveFlag::Capture
                } else {
                    MoveFlag::Quiet
                };
                ml.push(Move::new(from, to, flag));
            }
        }

        for from in pieces & pinned {
            let pin_mask = line_through(king_sq, from);
            for to in attacks(from) & target & pin_mask {
                let flag = if enemy.contains(to) {
                    MoveFlag::Capture
                } else {
                    MoveFlag::Quiet
                };
                ml.push(Move::new(from, to, flag));
            }
        }
    }

    fn collect_castling(&self, ml: &mut MoveList) {
        let stm = self.side_to_move();
        let occ = self.occupancies();
        let enemy_threats = self.board_state.threats_by[!stm];

        if self.checkers().not_empty() {
            return; // can't castle out of check
        }

        match stm {
            Color::White => {
                if self
                    .board_state
                    .castling
                    .is_allowed(CastlingDirections::WhiteKingside)
                    && (Bitboard(0x60) & occ).is_empty()
                    && (Bitboard(0x60) & enemy_threats).is_empty()
                {
                    ml.push(Move::new(Square::E1, Square::G1, MoveFlag::CastleKingside));
                }
                if self
                    .board_state
                    .castling
                    .is_allowed(CastlingDirections::WhiteQueenside)
                    && (Bitboard(0x0E) & occ).is_empty()
                    && (Bitboard(0x0C) & enemy_threats).is_empty()
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
                    && (Bitboard(0x6000000000000000) & enemy_threats).is_empty()
                {
                    ml.push(Move::new(Square::E8, Square::G8, MoveFlag::CastleKingside));
                }
                if self
                    .board_state
                    .castling
                    .is_allowed(CastlingDirections::BlackQueenside)
                    && (Bitboard(0x0E00000000000000) & occ).is_empty()
                    && (Bitboard(0x0C00000000000000) & enemy_threats).is_empty()
                {
                    ml.push(Move::new(Square::E8, Square::C8, MoveFlag::CastleQueenside));
                }
            }
        }
    }

    fn collect_pawn_moves<G: GenType>(
        &self,
        ml: &mut MoveList,
        target: Bitboard,
        pinned: Bitboard,
    ) {
        let stm = self.side_to_move();
        let up = UP_DIR[stm];
        let pawns = self.colored_pieces(stm, PieceType::Pawn);
        let empty = !self.occupancies();
        let enemy = self.get_color(!stm);
        let king_sq = self.king_square(stm);
        let start_rank = PAWN_START[stm];

        if !G::CAPTURES_ONLY {
            // A pawn pinned along the king's file can still push straight ahead
            // (the pin ray must be vertical if it shares a file with the king);
            // any other pin direction forbids pushing at all.
            let pushable = pawns & (!pinned | Bitboard::file(king_sq.get_file()));

            for from in pushable {
                let one = from.shift(up);
                if !empty.contains(one) {
                    continue;
                }
                if Bitboard::BOTH_HOME_ROWS.contains(one) {
                    if target.contains(one) {
                        ml.push(Move::new(from, one, MoveFlag::PromoQueen));

                        ml.push(Move::new(from, one, MoveFlag::PromoRook));
                        ml.push(Move::new(from, one, MoveFlag::PromoBishop));
                        ml.push(Move::new(from, one, MoveFlag::PromoKnight));
                    }
                } else {
                    if target.contains(one) {
                        ml.push(Move::new(from, one, MoveFlag::Quiet));
                    }
                    if from.get_rank() == start_rank {
                        let two = one.shift(up);
                        if empty.contains(two) && target.contains(two) {
                            ml.push(Move::new(from, two, MoveFlag::DoublePush));
                        }
                    }
                }
            }
        }

        for from in pawns {
            let pin_mask = if pinned.contains(from) {
                line_through(king_sq, from)
            } else {
                Bitboard::ALL
            };
            for to in get_pawn_attacks(from, stm) & enemy & target & pin_mask {
                if Bitboard::BOTH_HOME_ROWS.contains(to) {
                    ml.push(Move::new(from, to, MoveFlag::PromoCaptureQueen));
                    ml.push(Move::new(from, to, MoveFlag::PromoCaptureRook));
                    ml.push(Move::new(from, to, MoveFlag::PromoCaptureBishop));
                    ml.push(Move::new(from, to, MoveFlag::PromoCaptureKnight));
                } else {
                    ml.push(Move::new(from, to, MoveFlag::Capture));
                }
            }
        }

        // En passant is the one case still resolved via a bespoke legality
        // check rather than pin/target masks — see is_legal.rs's
        // discovered-check-on-the-rank handling.
        let ep = self.en_passant();
        if ep != Square::None {
            for from in get_pawn_attacks(ep, !stm) & pawns {
                let mv = Move::new(from, ep, MoveFlag::EnPassant);
                if self.is_legal(mv) {
                    ml.push(mv);
                }
            }
        }
    }
}
