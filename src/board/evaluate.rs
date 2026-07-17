use crate::types::color::Color;

impl super::Board {
    pub fn evaluate(&self) -> i32 {
        let score = self.material();
        if self.side_to_move() == Color::White {
            score
        } else {
            -score
        }
    }
}
