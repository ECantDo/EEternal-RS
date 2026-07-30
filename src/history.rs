use crate::types::bitboard::Bitboard;
use crate::types::color::Color;
use crate::types::moves::Move;

// =================================================================================================
// Types/Helpers
// =================================================================================================
type MoveHistory<T> = [[T; 64]; 64];

fn zeroed_box<T>() -> Box<T> {
    unsafe {
        let layout = std::alloc::Layout::new::<T>();
        let ptr = std::alloc::alloc_zeroed(layout);
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        Box::<T>::from_raw(ptr.cast())
    }
}

// =================================================================================================
// Quiet History
// =================================================================================================
pub struct QuietHistory {
    // [side to move][is from threatened <false/true>][ is to threatened <false/true>][from][to]
    entries: Box<[[[MoveHistory<i16>; 2]; 2]; 2]>,
}

impl QuietHistory {
    const MAX_HISTORY: i32 = 8192; // ceil(i16::MAX / 4) -> 32768 / 4 (max is 32767)

    pub fn get(&self, threats: Bitboard, stm: Color, mv: Move) -> i32 {
        self.entries[stm][threats.contains(mv.from()) as usize][threats.contains(mv.to()) as usize]
            [mv.from()][mv.to()] as i32
    }

    pub fn update(&mut self, threats: Bitboard, stm: Color, mv: Move, bonus: i32) {
        let entry = &mut self.entries[stm][threats.contains(mv.from()) as usize]
            [threats.contains(mv.to()) as usize][mv.from()][mv.to()];
        let bonus = bonus.clamp(-Self::MAX_HISTORY, Self::MAX_HISTORY);
        *entry += (bonus - bonus.abs() * (*entry) as i32 / Self::MAX_HISTORY) as i16;
    }
}

impl Default for QuietHistory {
    fn default() -> Self {
        Self {
            entries: zeroed_box(),
        }
    }
}
