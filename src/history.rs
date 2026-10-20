use crate::board::Board;
use crate::types::UP_DIR;
use crate::types::bitboard::Bitboard;
use crate::types::color::Color;
use crate::types::moves::Move;
use crate::types::piece::{Piece, PieceType};
use crate::types::square::Square;

// =================================================================================================
// Types/Helpers
// =================================================================================================
type FromToHistory<T> = [[T; 64]; 64];
type PieceToHistory<T> = [[T; 64]; Piece::NUM + 1];

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

/// Back to how `zeroed_box` made it, without reallocating
fn zero_in_place<T>(b: &mut Box<T>) {
	// SAFETY: only used on the integer tables `zeroed_box` makes, whose fresh state is all-zero bytes
	unsafe { std::ptr::write_bytes(&mut **b as *mut T, 0, 1) }
}

fn apply_bonus<const MAX: i32>(entry: &mut i16, bonus: i32) {
	let bonus = bonus.clamp(-MAX, MAX);
	*entry += (bonus - bonus.abs() * (*entry) as i32 / MAX) as i16;
}

// =================================================================================================
// Quiet History
// =================================================================================================
pub struct QuietHistory {
	// [side to move][is from threatened <false/true>][ is to threatened <false/true>][from][to]
	entries: Box<[[[FromToHistory<i16>; 2]; 2]; 2]>,
}

impl QuietHistory {
	const MAX_HISTORY: i32 = crate::params::QUIET_HISTORY_MAX; // was ceil(i16::MAX / 4) -> 32768 / 4 (max is 32767)

	pub fn get(&self, threats: Bitboard, stm: Color, mv: Move) -> i32 {
		self.entries[stm][threats.contains(mv.from()) as usize][threats.contains(mv.to()) as usize]
			[mv.from()][mv.to()] as i32
	}

	pub fn update(&mut self, threats: Bitboard, stm: Color, mv: Move, bonus: i32) {
		let entry = &mut self.entries[stm][threats.contains(mv.from()) as usize]
			[threats.contains(mv.to()) as usize][mv.from()][mv.to()];
		apply_bonus::<{ Self::MAX_HISTORY }>(entry, bonus);
	}
}

impl QuietHistory {
	/// Fresh (all zero) again, reusing the allocation
	pub fn clear(&mut self) {
		zero_in_place(&mut self.entries);
	}
}

impl Default for QuietHistory {
	fn default() -> Self {
		Self {
			entries: zeroed_box(),
		}
	}
}
// =================================================================================================
// Noisy History
// =================================================================================================

pub struct NoisyHistory {
	// [piece][to][captured_piece_type][to_threatened?]
	entries: Box<PieceToHistory<[[i16; 2]; 7]>>,
}

impl NoisyHistory {
	const MAX_HISTORY: i32 = crate::params::NOISY_HISTORY_MAX;

	pub fn get(&self, threats: Bitboard, piece: Piece, sq: Square, captured: PieceType) -> i32 {
		self.entries[piece][sq][captured][threats.contains(sq) as usize] as i32
	}

	pub fn update(
		&mut self,
		threats: Bitboard,
		piece: Piece,
		sq: Square,
		captured: PieceType,
		bonus: i32,
	) {
		let entry = &mut self.entries[piece][sq][captured][threats.contains(sq) as usize];
		apply_bonus::<{ Self::MAX_HISTORY }>(entry, bonus);
	}
}

impl NoisyHistory {
	/// Fresh (all zero) again, reusing the allocation
	pub fn clear(&mut self) {
		zero_in_place(&mut self.entries);
	}
}

impl Default for NoisyHistory {
	fn default() -> Self {
		Self {
			entries: zeroed_box(),
		}
	}
}

pub fn noisy_key(board: &Board, mv: Move, stm: Color) -> (PieceType, Square) {
	if mv.is_en_passant() {
		(PieceType::Pawn, mv.to().shift(-UP_DIR[stm]))
	} else {
		(board.get_piece_on_square(mv.to()).piece_type(), mv.to())
	}
}

// =================================================================================================
// Correction History
// =================================================================================================
pub struct CorrectionHistory {
	entries: Box<[[i16; Self::SIZE]; 2]>, // [stm][pawn_key & mask]
}

impl CorrectionHistory {
	const SIZE: usize = 1 << 16;
	const MASK: usize = Self::SIZE - 1;
	const MAX_HISTORY: i32 = crate::params::CORRHIST_MAX;

	pub fn get(&self, stm: Color, pawn_key: u64) -> i32 {
		self.entries[stm][pawn_key as usize & Self::MASK] as i32
	}

	pub fn update(&mut self, stm: Color, pawn_key: u64, bonus: i32) {
		let entry = &mut self.entries[stm][pawn_key as usize & Self::MASK];
		apply_bonus::<{ Self::MAX_HISTORY }>(entry, bonus);
	}
}

impl CorrectionHistory {
	/// Fresh (all zero) again, reusing the allocation
	pub fn clear(&mut self) {
		zero_in_place(&mut self.entries);
	}
}

impl Default for CorrectionHistory {
	fn default() -> Self {
		Self {
			entries: zeroed_box(),
		}
	}
}
