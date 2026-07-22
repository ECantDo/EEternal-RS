use crate::attacking::{get_bishop_attacks, get_rook_attacks};
use crate::board::Board;
use crate::rays::{between, line_through};
use crate::types::moves::Move;
use crate::types::piece::PieceType;
use crate::types::UP_DIR;

impl Board{
    pub fn is_legal(&self, mv: Move) -> bool {
        let stm = self.side_to_move();
        let king = self.king_square(stm);
        let from = mv.from();
        let to = mv.to();

        if self.get_piece_on_square(from).piece_type() == PieceType::King {
            if mv.is_castling() {
                
                return true; // path/attacked-square checks already done at generation time
            }
            return !self.board_state.threats_by[!stm].contains(to);
        }

        let checkers = self.checkers();
        if checkers.not_empty() {
            if checkers.popcount() > 1 {
                return false; // only the king escapes double check
            }
            let checker_sq = checkers.lsb();
            // For en passant, the square that actually resolves the check is
            // the captured pawn's square, not the empty landing square.
            let relevant_sq = if mv.is_en_passant() {
                to.shift(-UP_DIR[stm])
            } else {
                to
            };
            if !(checkers | between(king, checker_sq)).contains(relevant_sq) {
                return false;
            }
        }

        if mv.is_en_passant() {
            // The classic case a plain pin check can't catch: both the captured
            // and moving pawn vanish from the same rank, opening a rook/queen
            // attack on the king that wasn't there with either pawn still on it.
            let cap_sq = to.shift(-UP_DIR[stm]);
            let occ = self.occupancies() ^ from.to_bitboard() ^ to.to_bitboard() ^ cap_sq.to_bitboard();
            let diagonal = self.colored_pieces(!stm, PieceType::Bishop) | self.colored_pieces(!stm, PieceType::Queen);
            let orthogonal = self.colored_pieces(!stm, PieceType::Rook) | self.colored_pieces(!stm, PieceType::Queen);
            if (get_bishop_attacks(king, occ) & diagonal).not_empty()
                || (get_rook_attacks(king, occ) & orthogonal).not_empty()
            {
                return false;
            }
            return true;
        }

        if self.pinned(stm).contains(from) {
            return line_through(king, from).contains(to);
        }

        true
    }
}