use super::{moves::Move, MAX_MOVES};
use crate::types::score::Score;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MoveEntry {
    ordering_score: i32, // 4
    mv: Move,            // 2
}

impl MoveEntry {
    pub fn default() -> Self {
        Self {
            mv: Move::NONE,
            ordering_score: Score::ZERO,
        }
    }

    pub fn mv(&self) -> Move {
        self.mv
    }

    pub fn score(&self) -> i32 {
        self.ordering_score
    }
}
pub struct MoveList {
    moves: [MoveEntry; MAX_MOVES],
    len: usize,
}

impl MoveList {
    pub fn new() -> Self {
        Self {
            moves: [MoveEntry::default(); MAX_MOVES],
            len: 0,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn clear(&mut self) {
        self.len = 0
    }

    pub fn push(&mut self, mv: Move) {
        debug_assert!(self.len < MAX_MOVES, "Move list overflow");
        self.moves[self.len] = MoveEntry {
            mv,
            ordering_score: Score::ZERO,
        };
        self.len += 1;
    }

    pub fn pop(&mut self) -> Move {
        debug_assert!(self.len > 0);
        self.len -= 1;
        self.moves[self.len].mv
    }

    pub fn get(&self, idx: usize) -> MoveEntry {
        debug_assert!(idx < self.len);
        self.moves[idx]
    }

    /// Swap remove
    pub fn remove(&mut self, idx: usize) -> Move {
        debug_assert!(idx < self.len);
        let move_entry = self.moves[idx];
        self.len -= 1;
        self.moves[idx] = self.moves[self.len];
        move_entry.mv
    }

    pub fn place_first(&mut self, idx: usize) {
        debug_assert!(idx < self.len);
        if idx == 0 {
            return;
        }

        let mv = self.moves[idx];
        self.moves.copy_within(0..idx, 1);
        self.moves[0] = mv;
    }

    pub fn swap(&mut self, idx1: usize, idx2: usize) {
        debug_assert!(idx1 < self.len && idx2 < self.len);

        self.moves.swap(idx1, idx2);
    }

    pub fn set_score(&mut self, idx: usize, score: i32) {
        debug_assert!(idx < self.len);
        self.moves[idx].ordering_score = score;
    }
}

impl std::ops::Index<usize> for MoveList {
    type Output = MoveEntry;
    fn index(&self, i: usize) -> &Self::Output {
        &self.moves[i]
    }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = MoveEntry;
    type IntoIter = std::iter::Take<std::array::IntoIter<Self::Item, MAX_MOVES>>;
    fn into_iter(self) -> Self::IntoIter {
        self.moves.into_iter().take(self.len)
    }
}
