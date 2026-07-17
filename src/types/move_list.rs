use super::{MAX_MOVES, moves::Move};

pub struct MoveList {
    moves: [Move; MAX_MOVES],
    len: usize,
}

impl MoveList {
    pub const fn new() -> Self {
        Self {
            moves: [Move::NONE; MAX_MOVES],
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
        self.moves[self.len] = mv;
        self.len += 1;
    }

    pub fn pop(&mut self) -> Move {
        debug_assert!(self.len > 0);
        self.len -= 1;
        self.moves[self.len]
    }

    /// Swap remove
    pub fn remove(&mut self, idx: usize) -> Move {
        debug_assert!(idx < self.len);
        let mv = self.moves[idx];
        self.len -= 1;
        self.moves[idx] = self.moves[self.len];
        mv
    }
}

impl std::ops::Index<usize> for MoveList {
    type Output = Move;
    fn index(&self, i: usize) -> &Move {
        &self.moves[i]
    }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = Move;
    type IntoIter = std::iter::Take<std::array::IntoIter<Move, MAX_MOVES>>;
    fn into_iter(self) -> Self::IntoIter {
        self.moves.into_iter().take(self.len)
    }
}
