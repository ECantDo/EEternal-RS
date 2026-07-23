use crate::types::color::Color;
use crate::types::piece::PieceType;

impl super::Board {
    pub fn evaluate(&self) -> i32 {
        let mut score: i32 = 0;

        for color in Color::ALL {
            for piece_type in PieceType::ALL {
                score +=
                    (self.colored_pieces(color, piece_type).popcount() as i32) * piece_type.value();
            }
            // Flip 1 -> white becomes negative
            // Flip 2 -> white becomes positive (black negative)
            score = -score;
        }
        if self.side_to_move() == Color::White {
            score
        } else {
            -score
        }
    }
}
