use crate::{
    attacking::{
        get_bishop_attacks, get_king_attacks, get_knight_attacks, get_pawn_attacks,
        get_rook_attacks,
    },
    board::Board,
    types::{
        bitboard::Bitboard,
        color::Color,
        moves::Move,
        piece::{Piece, PieceType},
        square::Square,
        UP_DIR,
    },
};

/// Material values used only for SEE — a separate scale from evaluation.
/// The king gets a large sentinel so a king "recapture" is never treated as
/// a good trade inside the exchange, even though its real eval value is 0.
fn see_value(pt: PieceType) -> i32 {
    match pt {
        PieceType::Pawn => 100,
        PieceType::Knight => 300,
        PieceType::Bishop => 400,
        PieceType::Rook => 600,
        PieceType::Queen => 1100,
        PieceType::King => 20000,
        PieceType::None => 0,
    }
}

impl Board {
    /// All pieces of either color attacking `square`, given a (possibly
    /// hypothetical) occupancy `occ`. Used to walk the exchange on `square`
    /// one virtual capture at a time without mutating the real board.
    fn attackers_to(&self, square: Square, occ: Bitboard) -> Bitboard {
        let bishops_queens = self.pieces(PieceType::Bishop) | self.pieces(PieceType::Queen);
        let rooks_queens = self.pieces(PieceType::Rook) | self.pieces(PieceType::Queen);

        let white_pawn_attackers = get_pawn_attacks(square, Color::Black)
            & self.colored_pieces(Color::White, PieceType::Pawn);
        let black_pawn_attackers = get_pawn_attacks(square, Color::White)
            & self.colored_pieces(Color::Black, PieceType::Pawn);

        (white_pawn_attackers | black_pawn_attackers)
            | (get_knight_attacks(square) & self.pieces(PieceType::Knight))
            | (get_king_attacks(square) & self.pieces(PieceType::King))
            | (get_bishop_attacks(square, occ) & bishops_queens)
            | (get_rook_attacks(square, occ) & rooks_queens)
    }

    /// The least valuable piece of `color` in `attackers`, if any.
    fn least_valuable_attacker(
        &self,
        attackers: Bitboard,
        color: Color,
    ) -> Option<(Square, PieceType)> {
        const ORDER: [PieceType; 6] = [
            PieceType::Pawn,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Rook,
            PieceType::Queen,
            PieceType::King,
        ];

        for &pt in &ORDER {
            let mine = attackers & self.colored_pieces(color, pt);
            if mine.not_empty() {
                return Some((mine.lsb(), pt));
            }
        }
        None
    }

    /// Static Exchange Evaluation: the net material swing (in the mover's
    /// favor) if the exchange on `mv.to()` plays out with both sides always
    /// recapturing with their least valuable attacker. Call this on the
    /// board *before* `mv` is made.
    ///
    /// Returns 0 for non-captures.
    pub fn see(&self, mv: Move) -> i32 {
        let from = mv.from();
        let to = mv.to();

        let captured = self.get_piece_on_square(to);
        if captured == Piece::None && !mv.is_en_passant() {
            return 0; // not a capture
        }

        let mut gain = [0i32; 32];
        let mut depth: usize = 0;

        gain[0] = if mv.is_en_passant() {
            see_value(PieceType::Pawn)
        } else {
            see_value(captured.piece_type())
        };

        let mut occ = self.occupancies();
        occ.clear(from);

        if mv.is_en_passant() {
            let stm = self.side_to_move();
            let cap_sq = to.shift(-UP_DIR[stm]);
            occ.clear(cap_sq);
        }

        let mut attackers = self.attackers_to(to, occ) & occ;
        let mut attacker_type = self.get_piece_on_square(from).piece_type();
        let mut side = !self.side_to_move(); // opponent recaptures first

        while depth < 30 {
            depth += 1;
            gain[depth] = see_value(attacker_type) - gain[depth - 1];

            // If even winning this piece outright doesn't help, stop early.
            if (-gain[depth - 1]).max(gain[depth]) < 0 {
                break;
            }

            let Some((attacker_sq, pt)) = self.least_valuable_attacker(attackers, side) else {
                break;
            };

            occ.clear(attacker_sq);
            attackers = self.attackers_to(to, occ) & occ;

            attacker_type = pt;
            side = !side;
        }

        while depth > 0 {
            gain[depth - 1] = -(-gain[depth - 1]).max(gain[depth]);
            depth -= 1;
        }

        gain[0]
    }
}
