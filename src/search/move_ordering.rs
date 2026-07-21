use crate::types::move_list::MoveEntry;
use crate::{
    search::search_types::SearchData,
    types::{move_list::MoveList, moves::Move},
};

pub struct OrderedMoves<'a> {
    move_list: &'a mut MoveList,
    idx: usize,
}

pub const CAPTURE_VALUE: i32 = 1_000_000;

impl<'a> OrderedMoves<'a> {
    pub fn new(move_list: &'a mut MoveList) -> Self {
        Self { move_list, idx: 0 }
    }

    pub fn score_moves(&mut self, search_data: &SearchData, tt_move: Move) {
        for idx in 0..self.move_list.len() {
            let mv = self.move_list.get(idx).mv();
            let score = if mv == tt_move {
                i32::MAX
            } else if mv.is_capture() {
                CAPTURE_VALUE + search_data.board.see(mv)
            } else {
                0
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
