use crate::types::move_list::MoveEntry;
use crate::types::piece::PieceType;
use crate::{
    search::search_types::SearchData,
    types::{move_list::MoveList, moves::Move},
};

pub struct OrderedMoves<'a> {
    move_list: &'a mut MoveList,
    idx: usize,
}

pub const CAPTURE_VALUE: i32 = 1_000_000;
pub const KILLER_VALUE: i32 = 900_000;

impl<'a> OrderedMoves<'a> {
    pub fn new(move_list: &'a mut MoveList) -> Self {
        Self { move_list, idx: 0 }
    }

    pub fn score_moves(&mut self, search_data: &SearchData, tt_move: Move, ply: i32) {
        let killers = search_data.killers.get(ply);

        for idx in 0..self.move_list.len() {
            let mv = self.move_list.get(idx).mv();
            let score = if mv == tt_move {
                debug_assert!(mv != Move::NONE);
                i32::MAX
            } else if mv.is_capture() {
                CAPTURE_VALUE + search_data.board.see(mv)
            } else if mv == killers[0] {
                KILLER_VALUE + 10
            } else if mv == killers[1] {
                KILLER_VALUE
            } else {
                score_quiet(search_data, mv, ply)
            };
            self.move_list.set_score(idx, score);
        }
    }

    pub fn select_next_best(&mut self) -> MoveEntry {
        debug_assert!(self.idx < self.move_list.len());
        let mut best_idx = self.idx;
        let mut best_move = self.move_list.get(best_idx);

        let mut i = self.idx + 1;
        while i < self.move_list.len() {
            if self.move_list.get(i).score() > best_move.score() {
                best_idx = i;
                best_move = self.move_list.get(i);
            }
            i += 1;
        }

        self.move_list.swap(self.idx, best_idx);
        self.idx += 1;
        best_move
    }
}

impl<'a> Iterator for OrderedMoves<'a> {
    type Item = MoveEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.move_list.len() {
            return None;
        }
        Some(self.select_next_best())
    }
}

fn score_quiet(search_data: &SearchData, mv: Move, ply: i32) -> i32 {
    // Get their threats ... haven't made the move yet, so look to where they are attacking
    let my_side = search_data.board.side_to_move();
    let threats = search_data.board.attacks_by_color(!my_side);
    // let occ = search_data.board.occupancies();

    // Pawn, Knight, Bishop, Rook, Queen, King
    // TODO: This
    // let escape_attack_score: [i32; PieceType::NUM] = [0, 8000, 8000, 15000, 20000, 0];
    // let pt: PieceType = search_data.board.get_piece_on_square(mv.from()).piece_type();
    // score += escape[pt] * <is threatened?>

    search_data.quiet_history.get(threats, my_side, mv)
}
