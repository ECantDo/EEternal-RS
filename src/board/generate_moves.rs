use crate::types::moves::Move;
use crate::types::{PAWN_START, UP_DIR};
use crate::{
    attacking::{
        get_bishop_attacks, get_king_attacks, get_knight_attacks, get_pawn_attacks,
        get_queen_attacks, get_rook_attacks,
    },
    types::{
        File,
        bitboard::Bitboard,
        move_list::MoveList,
        moves::MoveFlag,
        piece::{Piece, PieceType},
        square::Square,
    },
};

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum MoveGenType {
    Quiet,
    Captures,
}

impl super::Board {
    pub fn generate_all_pseudolegal_moves(&self, move_type: MoveGenType) -> MoveList {
        let mut ml = MoveList::new();
        self.append_all_pseudolegal_moves(&mut ml);
        ml
    }

    pub fn append_all_pseudolegal_moves(&self, ml: &mut MoveList) {
        let stm = self.side_to_move();
        let mut target = Bitboard::ALL; // Can replace with their peices to get caputres
        // Not going to worry about captures yet

        target &= !self.get_color(stm); // Remove my pieces from `to` locations

        self.gen_pseudolegal(
            ml,
            target,
            self.colored_pieces(stm, PieceType::Knight),
            get_knight_attacks,
        );
        self.gen_pseudolegal(
            ml,
            target,
            self.colored_pieces(stm, PieceType::King),
            get_king_attacks,
        );
        self.gen_pseudolegal(
            ml,
            target,
            self.colored_pieces(stm, PieceType::Bishop),
            |sq| get_bishop_attacks(sq, self.occupancies()),
        );
        self.gen_pseudolegal(
            ml,
            target,
            self.colored_pieces(stm, PieceType::Rook),
            |sq| get_rook_attacks(sq, self.occupancies()),
        );
        self.gen_pseudolegal(
            ml,
            target,
            self.colored_pieces(stm, PieceType::Queen),
            |sq| get_queen_attacks(sq, self.occupancies()),
        );
        self.gen_pawn_pseudolegal(ml);
    }

    fn gen_pseudolegal<F: Fn(Square) -> Bitboard>(
        &self,
        ml: &mut MoveList,
        target: Bitboard,
        piece_bitboard: Bitboard,
        attacks: F,
    ) {
        for from in piece_bitboard {
            let moves: Bitboard = attacks(from);

            for to in moves & target {
                let flag = if self.get_piece_on_square(to) == Piece::None {
                    MoveFlag::Quiet
                } else {
                    MoveFlag::Capture
                };
                ml.push(Move::new(from, to, flag));
            }
        }
    }

    fn gen_pawn_pseudolegal(&self, ml: &mut MoveList) {
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
}
