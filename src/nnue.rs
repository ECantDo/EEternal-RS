// Architecture ported from Reckless: the accumulator lives on the search thread
// (as a `Network`), not on `Board`. `push`/`pop` just record *what* is about to
// change; the actual feature add/remove math only happens lazily inside
// `evaluate()`, which walks back to the last accurate ply on the stack and
// replays deltas forward. Internal search nodes that never call `evaluate()`
// (i.e. every non-leaf node) pay zero NNUE cost.
//
// Two nets, the Stockfish way: a big one and a small one, both baked into the
// binary, same topology apart from the hidden size:
//   (768x4hm -> H)x2 -> 8, SCReLU, H = 1024 (big) or 512 (small)
// The big one is stronger per node; the small one is about 1.4x as fast, which
// should be the better trade without AVX2 (the scalar path), so that's the
// default there. UCI option `Net` (big | small) overrides it.
//
// Inputs: horizontally mirrored, with 4 king buckets, the same as bullet's
// `ChessBucketsMirrored::new(KING_BUCKETS_4)` (bullet/examples/eeternal_viri.rs).
// Each perspective whose king is on files e-h sees the board flipped
// left-to-right (`square ^ 7`), and picks one of 4 blocks of input weights from
// its own king's square (see `KING_BUCKET_LAYOUT`). A king move that crosses the
// d/e line or into another bucket changes every feature of its perspective, so
// that's the one "can't update, must refresh" case: `ensure_accurate` refreshes
// from the board instead of replaying.
//
// Output: 8 buckets by piece count, `(pieces - 2) / 4`.

use std::alloc::{Layout, alloc_zeroed, handle_alloc_error};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use crate::{
	board::Board,
	types::{
		MAX_PLY, UP_DIR,
		castling::castling_rook_squares,
		color::Color,
		moves::Move,
		piece::{Piece, PieceType},
		square::Square,
	},
};

pub const INPUT_SIZE: usize = 768;
pub const KING_BUCKETS: usize = 4;
/// Rows of input weights: one block of 768 per king bucket
pub const INPUT_ROWS: usize = INPUT_SIZE * KING_BUCKETS;
pub const OUTPUT_BUCKETS: usize = 8;
pub const BIG_HIDDEN: usize = 1024;
pub const SMALL_HIDDEN: usize = 512;
pub const QA: i32 = 255; // don't go above 255 (i16 squared must stay < 32768 headroom-safe)
pub const QB: i32 = 64;
pub const SCALE: i32 = 400;

const _: () = assert!(BIG_HIDDEN % 16 == 0 && SMALL_HIDDEN % 16 == 0);

/// King bucket by the king's square seen from its own side (black's is `sq ^ 56`), files mirrored to a-d (so g1
/// counts as b1). Rank 1 corner / centre, rank 2, then everything further up. Must match the trainer's KING_BUCKETS_4.
#[rustfmt::skip]
const KING_BUCKET_LAYOUT: [usize; 32] = [
	0, 0, 1, 1, // rank 1: a/b (castled, mirrored g/h) | c/d (centre, mirrored e/f)
	2, 2, 2, 2, // rank 2
	3, 3, 3, 3, // ranks 3-8
	3, 3, 3, 3,
	3, 3, 3, 3,
	3, 3, 3, 3,
	3, 3, 3, 3,
	3, 3, 3, 3,
];

// =====================================================================================================================
// Which net
// =====================================================================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NetSize {
	Big,
	Small,
}

impl NetSize {
	pub const fn name(self) -> &'static str {
		match self {
			NetSize::Big => "big",
			NetSize::Small => "small",
		}
	}

	pub fn parse(s: &str) -> Option<Self> {
		match s.to_ascii_lowercase().as_str() {
			"big" => Some(NetSize::Big),
			"small" => Some(NetSize::Small),
			_ => None,
		}
	}
}

/// The big net with AVX2, the small one on the scalar path (`target_feature` is a compile-time flag: the release
/// builds for x86-64-v2 have no AVX2, `target-cpu=native` builds on an AVX2 machine do)
pub const DEFAULT_NET: NetSize = if cfg!(target_feature = "avx2") {
	NetSize::Big
} else {
	NetSize::Small
};

static USE_SMALL: AtomicBool = AtomicBool::new(matches!(DEFAULT_NET, NetSize::Small));

/// The net searches use from now on (`NNUE::reset` picks it up)
pub fn set_net(net: NetSize) {
	USE_SMALL.store(net == NetSize::Small, Ordering::Relaxed);
}

pub fn active_net() -> NetSize {
	if USE_SMALL.load(Ordering::Relaxed) {
		NetSize::Small
	} else {
		NetSize::Big
	}
}

// =====================================================================================================================
// Network parameters (weights/biases) - global, reference-counted, swappable at runtime
// =====================================================================================================================

/* `repr(C)` pins the field order/padding to match the raw file layout exactly
(inputWeights, then inputBiases, then outputWeights, then outputBias, all
i16, no gaps) - required for the straight byte copy in `params_from_bytes`.
`align(64)` rounds the size up to a multiple of 64, which bullet's padding of
the saved file matches. The output weights are [bucket][perspective][hidden]:
the same bytes as the file's [bucket][2 * hidden] rows (stm half first). */
#[repr(C, align(64))]
pub struct NNUEParams<const H: usize> {
	input_weights: [[i16; H]; INPUT_ROWS],
	input_biases: [i16; H],
	output_weights: [[[i16; H]; 2]; OUTPUT_BUCKETS],
	output_bias: [i16; OUTPUT_BUCKETS],
}

/// Place the trained networks at these paths (relative to this file); the
/// `embed-nnue` feature (on by default) bakes them into the binary.
/// EEternalRS_Datagen/net_check.sh finds them by the `-<hidden>x2` in the name.
#[cfg(feature = "embed-nnue")]
const BIG_BYTES: &[u8] = include_bytes!("nnue/768x4hm-1024x2-1-ob8.bin");
#[cfg(feature = "embed-nnue")]
const SMALL_BYTES: &[u8] = include_bytes!("nnue/768x4hm-512x2-1-ob8.bin");

#[cfg(feature = "embed-nnue")]
const _: () = assert!(
	BIG_BYTES.len() == size_of::<NNUEParams<BIG_HIDDEN>>(),
	"big net file is the wrong size"
);
#[cfg(feature = "embed-nnue")]
const _: () = assert!(
	SMALL_BYTES.len() == size_of::<NNUEParams<SMALL_HIDDEN>>(),
	"small net file is the wrong size"
);

static BIG_PARAMS: RwLock<Option<Arc<NNUEParams<BIG_HIDDEN>>>> = RwLock::new(None);
static SMALL_PARAMS: RwLock<Option<Arc<NNUEParams<SMALL_HIDDEN>>>> = RwLock::new(None);

/// Copies a raw network file straight into a fresh heap `NNUEParams` (the big
/// one is 6 MB, too big to build on the stack first). Little-endian, same as
/// the old C++ `loadFromPtr`/`memcpy`.
fn params_from_bytes<const H: usize>(bytes: &[u8]) -> Result<Arc<NNUEParams<H>>, String> {
	let expected_len = size_of::<NNUEParams<H>>();
	if bytes.len() != expected_len {
		return Err(format!(
			"NNUE file wrong size: got {} bytes, expected exactly {expected_len}",
			bytes.len()
		));
	}

	// SAFETY: `NNUEParams` is `#[repr(C)]` and made up entirely of `i16`
	// arrays (no niches), so zeroed memory is a valid value and overwriting it
	// with exactly `size_of::<NNUEParams>()` bytes is well-defined.
	let params = unsafe {
		let layout = Layout::new::<NNUEParams<H>>();
		let ptr = alloc_zeroed(layout) as *mut NNUEParams<H>;
		if ptr.is_null() {
			handle_alloc_error(layout);
		}
		std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, expected_len);
		Box::from_raw(ptr)
	};
	Ok(Arc::from(params))
}

/// Whether the net searches will use is loaded. Individual `NNUE` instances
/// snapshot the globals at construction/reset time (see `NNUE::new`), so this
/// is mainly useful for startup logging.
pub fn is_loaded() -> bool {
	match active_net() {
		NetSize::Big => BIG_PARAMS.read().unwrap().is_some(),
		NetSize::Small => SMALL_PARAMS.read().unwrap().is_some(),
	}
}

/// Mirrors `initNNUEEmbedded()` - loads both networks baked in at compile
/// time via the `embed-nnue` feature. Returns false (and loads nothing) if that
/// feature isn't enabled, same as "no embedded network found".
pub fn try_init_embedded() -> bool {
	#[cfg(feature = "embed-nnue")]
	{
		*BIG_PARAMS.write().unwrap() = Some(params_from_bytes(BIG_BYTES).unwrap());
		*SMALL_PARAMS.write().unwrap() = Some(params_from_bytes(SMALL_BYTES).unwrap());
		true
	}
	#[cfg(not(feature = "embed-nnue"))]
	{
		false
	}
}

/// Mirrors `initNNUE(filename)` - loads a network from disk at runtime, e.g.
/// via `setoption name EvalFile value <path>`. The file's size says whether
/// it's a big or a small net; it replaces that one, and searches switch to it.
pub fn init_from_file(path: &str) -> Result<NetSize, String> {
	let bytes =
		std::fs::read(path).map_err(|e| format!("Failed to open NNUE file '{path}': {e}"))?;
	let net = if bytes.len() == size_of::<NNUEParams<BIG_HIDDEN>>() {
		*BIG_PARAMS.write().unwrap() = Some(params_from_bytes(&bytes)?);
		NetSize::Big
	} else if bytes.len() == size_of::<NNUEParams<SMALL_HIDDEN>>() {
		*SMALL_PARAMS.write().unwrap() = Some(params_from_bytes(&bytes)?);
		NetSize::Small
	} else {
		return Err(format!(
			"NNUE file '{path}' is {} bytes, expected {} (big) or {} (small)",
			bytes.len(),
			size_of::<NNUEParams<BIG_HIDDEN>>(),
			size_of::<NNUEParams<SMALL_HIDDEN>>()
		));
	};
	set_net(net);
	Ok(net)
}

// =====================================================================================================================
// Accumulator
// =====================================================================================================================

#[derive(Clone, Copy)]
#[repr(C, align(64))]
struct Accumulator<const H: usize> {
	white: [i16; H],
	black: [i16; H],
}

impl<const H: usize> Default for Accumulator<H> {
	fn default() -> Self {
		Self {
			white: [0; H],
			black: [0; H],
		}
	}
}

/// Piece(0..11 via color/piece_type) x 64 squares:
/// White{Pawn..King} = 0..5, Black{Pawn..King} = 6..11.
fn feature_index(piece: Piece, square: Square) -> usize {
	let piece_type_idx = piece.piece_type() as usize;
	let color_idx = piece.color() as usize;
	(color_idx * PieceType::NUM + piece_type_idx) * 64 + square as usize
}

/// How one perspective sees the board, from its own king's square: mirrored or
/// not, and which king bucket's block of input weights it uses.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
struct KingSlot {
	/// The king is on files e-h, so every square's file is flipped
	flip: bool,
	/// First input row of the king bucket's block, `768 * bucket`
	offset: usize,
}

fn king_slot(color: Color, king: Square) -> KingSlot {
	let own = match color {
		Color::White => king as usize,
		Color::Black => king as usize ^ 56,
	};
	let (rank, file) = (own / 8, own % 8);
	let flip = file >= 4;
	let file = if flip { 7 - file } else { file };
	KingSlot {
		flip,
		offset: INPUT_SIZE * KING_BUCKET_LAYOUT[rank * 4 + file],
	}
}

/// Each perspective's slot (indexed by colour) in `board`.
fn king_slots(board: &Board) -> [KingSlot; 2] {
	[
		king_slot(Color::White, board.king_square(Color::White)),
		king_slot(Color::Black, board.king_square(Color::Black)),
	]
}

/// Feature add/remove and the output dot-product, with an AVX2 backend
/// selected at compile time and a portable scalar fallback otherwise. This is
/// the same split Reckless uses (`mod forward { mod vectorized; mod scalar; }`),
/// just without the neon/wasm branches - add those the same way if/when you
/// need them, following the `avx2` module as a template.
///
/// AVX2 only activates if the crate is actually *compiled* with it enabled -
/// `cfg(target_feature = "avx2")` reflects compile flags, not the host CPU.
/// Build with `RUSTFLAGS="-C target-feature=+avx2"` (or `target-cpu=native`,
/// or a `.cargo/config.toml` `[build] rustflags`) to turn this on; otherwise
/// you silently get the scalar path even on an AVX2 machine.
mod forward {
	use super::{Accumulator, KingSlot, NNUEParams, feature_index};
	use crate::types::{color::Color, piece::Piece, square::Square};

	/// The input row for white's perspective and for black's (colours swapped and
	/// the board flipped top-to-bottom), each flipped left-to-right when that
	/// perspective's king is on files e-h and offset to its king bucket's block.
	pub(super) fn indices(piece: Piece, square: Square, slots: [KingSlot; 2]) -> (usize, usize) {
		let white_sq = if slots[0].flip {
			square.flip_file()
		} else {
			square
		};
		let black_sq = if slots[1].flip {
			square.flip_rank().flip_file()
		} else {
			square.flip_rank()
		};
		let mirrored_piece = Piece::new(!piece.color(), piece.piece_type());
		(
			slots[0].offset + feature_index(piece, white_sq),
			slots[1].offset + feature_index(mirrored_piece, black_sq),
		)
	}

	pub fn add_feature<const H: usize>(
		acc: &mut Accumulator<H>,
		params: &NNUEParams<H>,
		piece: Piece,
		square: Square,
		slots: [KingSlot; 2],
	) {
		let (idx, midx) = indices(piece, square, slots);

		#[cfg(target_feature = "avx2")]
		unsafe {
			avx2::add(acc, params, idx, midx);
		}
		#[cfg(not(target_feature = "avx2"))]
		scalar::add(acc, params, idx, midx);
	}

	// ---- fused updates: `out = input + added - removed`, in one pass over the accumulator ----

	/// Quiet moves and quiet promotions: one feature added, one removed
	pub fn add_sub<const H: usize>(
		out: &mut Accumulator<H>,
		input: &Accumulator<H>,
		params: &NNUEParams<H>,
		add: (Piece, Square),
		sub: (Piece, Square),
		slots: [KingSlot; 2],
	) {
		let add = indices(add.0, add.1, slots);
		let sub = indices(sub.0, sub.1, slots);

		#[cfg(target_feature = "avx2")]
		unsafe {
			avx2::add_sub(out, input, params, add, sub);
		}
		#[cfg(not(target_feature = "avx2"))]
		scalar::add_sub(out, input, params, add, sub);
	}

	/// Captures, en passant and capture-promotions: one added, two removed
	pub fn add_sub2<const H: usize>(
		out: &mut Accumulator<H>,
		input: &Accumulator<H>,
		params: &NNUEParams<H>,
		add: (Piece, Square),
		sub1: (Piece, Square),
		sub2: (Piece, Square),
		slots: [KingSlot; 2],
	) {
		let add = indices(add.0, add.1, slots);
		let sub1 = indices(sub1.0, sub1.1, slots);
		let sub2 = indices(sub2.0, sub2.1, slots);

		#[cfg(target_feature = "avx2")]
		unsafe {
			avx2::add_sub2(out, input, params, add, sub1, sub2);
		}
		#[cfg(not(target_feature = "avx2"))]
		scalar::add_sub2(out, input, params, add, sub1, sub2);
	}

	/// Castling: king and rook both move, two added, two removed
	pub fn add2_sub2<const H: usize>(
		out: &mut Accumulator<H>,
		input: &Accumulator<H>,
		params: &NNUEParams<H>,
		add1: (Piece, Square),
		add2: (Piece, Square),
		sub1: (Piece, Square),
		sub2: (Piece, Square),
		slots: [KingSlot; 2],
	) {
		let add1 = indices(add1.0, add1.1, slots);
		let add2 = indices(add2.0, add2.1, slots);
		let sub1 = indices(sub1.0, sub1.1, slots);
		let sub2 = indices(sub2.0, sub2.1, slots);

		#[cfg(target_feature = "avx2")]
		unsafe {
			avx2::add2_sub2(out, input, params, add1, add2, sub1, sub2);
		}
		#[cfg(not(target_feature = "avx2"))]
		scalar::add2_sub2(out, input, params, add1, add2, sub1, sub2);
	}

	pub fn output<const H: usize>(
		acc: &Accumulator<H>,
		stm: Color,
		params: &NNUEParams<H>,
		bucket: usize,
	) -> i32 {
		#[cfg(target_feature = "avx2")]
		unsafe {
			avx2::output(acc, stm, params, bucket)
		}
		#[cfg(not(target_feature = "avx2"))]
		scalar::output(acc, stm, params, bucket)
	}

	// ---- scalar fallback: portable, relies on LLVM auto-vectorization ----
	#[cfg(not(target_feature = "avx2"))]
	mod scalar {
		use super::{Accumulator, NNUEParams};
		use crate::nnue::{QA, QB, SCALE};
		use crate::types::color::Color;

		pub fn add<const H: usize>(
			acc: &mut Accumulator<H>,
			params: &NNUEParams<H>,
			idx: usize,
			midx: usize,
		) {
			let weights = &params.input_weights[idx];
			for i in 0..H {
				acc.white[i] = acc.white[i].wrapping_add(weights[i]);
			}

			let mirrored_weights = &params.input_weights[midx];
			for i in 0..H {
				acc.black[i] = acc.black[i].wrapping_add(mirrored_weights[i]);
			}
		}

		// Fused updates. Wrapping math like the SIMD versions: an intermediate sum may overflow i16,
		// the final value is still right.
		pub fn add_sub<const H: usize>(
			out: &mut Accumulator<H>,
			input: &Accumulator<H>,
			params: &NNUEParams<H>,
			add: (usize, usize),
			sub: (usize, usize),
		) {
			let w = &params.input_weights;
			for i in 0..H {
				out.white[i] = input.white[i]
					.wrapping_add(w[add.0][i])
					.wrapping_sub(w[sub.0][i]);
			}
			for i in 0..H {
				out.black[i] = input.black[i]
					.wrapping_add(w[add.1][i])
					.wrapping_sub(w[sub.1][i]);
			}
		}

		pub fn add_sub2<const H: usize>(
			out: &mut Accumulator<H>,
			input: &Accumulator<H>,
			params: &NNUEParams<H>,
			add: (usize, usize),
			sub1: (usize, usize),
			sub2: (usize, usize),
		) {
			let w = &params.input_weights;
			for i in 0..H {
				out.white[i] = input.white[i]
					.wrapping_add(w[add.0][i])
					.wrapping_sub(w[sub1.0][i])
					.wrapping_sub(w[sub2.0][i]);
			}
			for i in 0..H {
				out.black[i] = input.black[i]
					.wrapping_add(w[add.1][i])
					.wrapping_sub(w[sub1.1][i])
					.wrapping_sub(w[sub2.1][i]);
			}
		}

		pub fn add2_sub2<const H: usize>(
			out: &mut Accumulator<H>,
			input: &Accumulator<H>,
			params: &NNUEParams<H>,
			add1: (usize, usize),
			add2: (usize, usize),
			sub1: (usize, usize),
			sub2: (usize, usize),
		) {
			let w = &params.input_weights;
			for i in 0..H {
				out.white[i] = input.white[i]
					.wrapping_add(w[add1.0][i])
					.wrapping_add(w[add2.0][i])
					.wrapping_sub(w[sub1.0][i])
					.wrapping_sub(w[sub2.0][i]);
			}
			for i in 0..H {
				out.black[i] = input.black[i]
					.wrapping_add(w[add1.1][i])
					.wrapping_add(w[add2.1][i])
					.wrapping_sub(w[sub1.1][i])
					.wrapping_sub(w[sub2.1][i]);
			}
		}

		pub fn output<const H: usize>(
			acc: &Accumulator<H>,
			stm: Color,
			params: &NNUEParams<H>,
			bucket: usize,
		) -> i32 {
			let (us, them) = match stm {
				Color::White => (&acc.white, &acc.black),
				Color::Black => (&acc.black, &acc.white),
			};

			let [w0, w1] = &params.output_weights[bucket];

			let mut sum: i32 = 0;
			for i in 0..H {
				let u = (us[i] as i32).clamp(0, QA);
				sum += (u * u) * (w0[i] as i32);

				let t = (them[i] as i32).clamp(0, QA);
				sum += (t * t) * (w1[i] as i32);
			}

			let mut output = sum / QA;
			output += params.output_bias[bucket] as i32;
			output *= SCALE;
			output /= QA * QB;

			output
		}
	}

	// ---- AVX2 backend: direct port of the original evaluateNNUE/addWeightsSIMD/
	// subWeightsSIMD intrinsics. Uses unaligned loads/stores (loadu/storeu) so it
	// doesn't depend on the accumulator/weight arrays having any particular
	// alignment - a few cycles slower per load than aligned intrinsics on paper,
	// but a non-issue on any CPU that actually has AVX2 (Sandy Bridge or newer).
	#[cfg(target_feature = "avx2")]
	mod avx2 {
		use super::{Accumulator, NNUEParams};
		use crate::nnue::{QA, QB, SCALE};
		use crate::types::color::Color;
		use std::arch::x86_64::*;

		#[target_feature(enable = "avx2")]
		unsafe fn add_half<const H: usize>(half: &mut [i16; H], weights: &[i16; H]) {
			unsafe {
				for i in (0..H).step_by(16) {
					let acc = _mm256_loadu_si256(half.as_ptr().add(i) as *const __m256i);
					let w = _mm256_loadu_si256(weights.as_ptr().add(i) as *const __m256i);
					let sum = _mm256_add_epi16(acc, w);
					_mm256_storeu_si256(half.as_mut_ptr().add(i) as *mut __m256i, sum);
				}
			}
		}

		#[target_feature(enable = "avx2")]
		unsafe fn add_sub_half<const H: usize>(
			out: &mut [i16; H],
			input: &[i16; H],
			w_add: &[i16; H],
			w_sub: &[i16; H],
		) {
			unsafe {
				for i in (0..H).step_by(16) {
					// Load in each of the values
					let acc = _mm256_loadu_si256(input.as_ptr().add(i) as *const __m256i);
					let add = _mm256_loadu_si256(w_add.as_ptr().add(i) as *const __m256i);
					let sub = _mm256_loadu_si256(w_sub.as_ptr().add(i) as *const __m256i);

					// Do math
					let res = _mm256_sub_epi16(_mm256_add_epi16(acc, add), sub);

					// Store it into the new acc (since it's in a stack)
					_mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, res);
				}
			}
		}

		#[target_feature(enable = "avx2")]
		unsafe fn add_sub2_half<const H: usize>(
			out: &mut [i16; H],
			input: &[i16; H],
			w_add: &[i16; H],
			w_sub1: &[i16; H],
			w_sub2: &[i16; H],
		) {
			unsafe {
				for i in (0..H).step_by(16) {
					// Load in each of the values
					let acc = _mm256_loadu_si256(input.as_ptr().add(i) as *const __m256i);
					let add = _mm256_loadu_si256(w_add.as_ptr().add(i) as *const __m256i);
					let sub1 = _mm256_loadu_si256(w_sub1.as_ptr().add(i) as *const __m256i);
					let sub2 = _mm256_loadu_si256(w_sub2.as_ptr().add(i) as *const __m256i);

					// Do math
					let res =
						_mm256_sub_epi16(_mm256_add_epi16(acc, add), _mm256_add_epi16(sub1, sub2));

					// Store it into the new acc (since it's in a stack)
					_mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, res);
				}
			}
		}
		#[target_feature(enable = "avx2")]
		unsafe fn add2_sub2_half<const H: usize>(
			out: &mut [i16; H],
			input: &[i16; H],
			w_add1: &[i16; H],
			w_add2: &[i16; H],
			w_sub1: &[i16; H],
			w_sub2: &[i16; H],
		) {
			unsafe {
				for i in (0..H).step_by(16) {
					// Load in each of the values
					let acc = _mm256_loadu_si256(input.as_ptr().add(i) as *const __m256i);
					let add1 = _mm256_loadu_si256(w_add1.as_ptr().add(i) as *const __m256i);
					let add2 = _mm256_loadu_si256(w_add2.as_ptr().add(i) as *const __m256i);
					let sub1 = _mm256_loadu_si256(w_sub1.as_ptr().add(i) as *const __m256i);
					let sub2 = _mm256_loadu_si256(w_sub2.as_ptr().add(i) as *const __m256i);

					// Do math
					let res = _mm256_sub_epi16(
						_mm256_add_epi16(acc, _mm256_add_epi16(add1, add2)),
						_mm256_add_epi16(sub1, sub2),
					);

					// Store it into the new acc (since it's in a stack)
					_mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, res);
				}
			}
		}

		pub unsafe fn add<const H: usize>(
			acc: &mut Accumulator<H>,
			params: &NNUEParams<H>,
			idx: usize,
			midx: usize,
		) {
			unsafe {
				add_half(&mut acc.white, &params.input_weights[idx]);
				add_half(&mut acc.black, &params.input_weights[midx]);
			}
		}

		// Fused wrappers: each feature is (white index, black index), see `forward::indices`
		pub unsafe fn add_sub<const H: usize>(
			out: &mut Accumulator<H>,
			input: &Accumulator<H>,
			params: &NNUEParams<H>,
			add: (usize, usize),
			sub: (usize, usize),
		) {
			let w = &params.input_weights;
			unsafe {
				add_sub_half(&mut out.white, &input.white, &w[add.0], &w[sub.0]);
				add_sub_half(&mut out.black, &input.black, &w[add.1], &w[sub.1]);
			}
		}

		pub unsafe fn add_sub2<const H: usize>(
			out: &mut Accumulator<H>,
			input: &Accumulator<H>,
			params: &NNUEParams<H>,
			add: (usize, usize),
			sub1: (usize, usize),
			sub2: (usize, usize),
		) {
			let w = &params.input_weights;
			unsafe {
				add_sub2_half(
					&mut out.white,
					&input.white,
					&w[add.0],
					&w[sub1.0],
					&w[sub2.0],
				);
				add_sub2_half(
					&mut out.black,
					&input.black,
					&w[add.1],
					&w[sub1.1],
					&w[sub2.1],
				);
			}
		}

		pub unsafe fn add2_sub2<const H: usize>(
			out: &mut Accumulator<H>,
			input: &Accumulator<H>,
			params: &NNUEParams<H>,
			add1: (usize, usize),
			add2: (usize, usize),
			sub1: (usize, usize),
			sub2: (usize, usize),
		) {
			let w = &params.input_weights;
			unsafe {
				add2_sub2_half(
					&mut out.white,
					&input.white,
					&w[add1.0],
					&w[add2.0],
					&w[sub1.0],
					&w[sub2.0],
				);
				add2_sub2_half(
					&mut out.black,
					&input.black,
					&w[add1.1],
					&w[add2.1],
					&w[sub1.1],
					&w[sub2.1],
				);
			}
		}

		/// One half's share of the SCReLU dot product, `sum(clamp(v)^2 * w)`, as
		/// `madd(v, v * w)`. `v * w` fits an i16 because v <= QA = 255 and the
		/// trainer clips the output weights to |w| <= 1.98 * QB = 126, so this is
		/// exactly the widen-then-multiply sum, in about half the instructions.
		#[target_feature(enable = "avx2")]
		unsafe fn screlu_dot<const H: usize>(acc: &[i16; H], weights: &[i16; H]) -> __m256i {
			let zero = _mm256_setzero_si256();
			let qa = _mm256_set1_epi16(QA as i16);
			let mut sum = _mm256_setzero_si256();
			unsafe {
				for i in (0..H).step_by(16) {
					let v = _mm256_loadu_si256(acc.as_ptr().add(i) as *const __m256i);
					let w = _mm256_loadu_si256(weights.as_ptr().add(i) as *const __m256i);
					let v = _mm256_min_epi16(_mm256_max_epi16(v, zero), qa);
					let vw = _mm256_mullo_epi16(v, w);
					sum = _mm256_add_epi32(sum, _mm256_madd_epi16(v, vw));
				}
			}
			sum
		}

		#[target_feature(enable = "avx2")]
		pub unsafe fn output<const H: usize>(
			acc: &Accumulator<H>,
			stm: Color,
			params: &NNUEParams<H>,
			bucket: usize,
		) -> i32 {
			let (us, them) = match stm {
				Color::White => (&acc.white, &acc.black),
				Color::Black => (&acc.black, &acc.white),
			};

			let [w0, w1] = &params.output_weights[bucket];
			let sum = unsafe { _mm256_add_epi32(screlu_dot(us, w0), screlu_dot(them, w1)) };

			// Horizontal reduction: 8 x i32 lanes -> scalar
			let lo = _mm256_castsi256_si128(sum);
			let hi = _mm256_extracti128_si256(sum, 1);
			let mut s = _mm_add_epi32(lo, hi);
			s = _mm_add_epi32(s, _mm_srli_si128(s, 8));
			s = _mm_add_epi32(s, _mm_srli_si128(s, 4));
			let mut output = _mm_cvtsi128_si32(s);

			output /= QA;
			output += params.output_bias[bucket] as i32;
			output *= SCALE;
			output /= QA * QB;

			output
		}
	}
}

// =====================================================================================================================
// Lazy per-ply stack
// =====================================================================================================================

/// Everything needed to replay one ply's worth of feature changes without
/// looking at the board: which move was played, what piece made it, and what
/// (if anything) it captured - captured purely from move state, mirroring
/// exactly what `Board::make_move` itself branches on.
#[derive(Clone, Copy, Default)]
struct MoveDelta {
	mv: Move,
	piece: Piece,
	captured: Piece,
}

impl MoveDelta {
	/// A king moving across the d/e line (mirroring) or into another king
	/// bucket changes every feature of its perspective.
	fn needs_refresh(self) -> bool {
		self.piece.piece_type() == PieceType::King
			&& king_slot(self.piece.color(), self.mv.from())
				!= king_slot(self.piece.color(), self.mv.to())
	}
}

#[derive(Clone, Copy, Default)]
struct StackEntry<const H: usize> {
	accumulator: Accumulator<H>,
	accurate: bool,
	delta: MoveDelta,
	/// Per colour: how that perspective sees the board (mirroring and king
	/// bucket). Only meaningful while `accurate`.
	slots: [KingSlot; 2],
}

/// One net's parameters and accumulator stack
struct Network<const H: usize> {
	parameters: Option<Arc<NNUEParams<H>>>,
	stack: Box<[StackEntry<H>]>,
	index: usize,
}

impl<const H: usize> Network<H> {
	fn new(parameters: Option<Arc<NNUEParams<H>>>) -> Self {
		Self {
			parameters,
			stack: vec![StackEntry::default(); MAX_PLY + 1].into_boxed_slice(),
			index: 0,
		}
	}

	fn reset(&mut self, board: &Board) {
		self.index = 0;
		self.full_refresh(board);
	}

	fn full_refresh(&mut self, board: &Board) {
		let Some(params) = self.parameters.clone() else {
			self.stack[self.index].accurate = true;
			return;
		};

		let entry = &mut self.stack[self.index];
		entry.accumulator.white = params.input_biases;
		entry.accumulator.black = params.input_biases;
		let slots = king_slots(board);

		for i in 0..Square::NUM as u8 {
			let square = Square::new(i);
			let piece = board.get_piece_on_square(square);
			if piece != Piece::None {
				forward::add_feature(&mut entry.accumulator, &params, piece, square, slots);
			}
		}

		entry.accurate = true;
		entry.slots = slots;
	}

	fn push(&mut self, delta: MoveDelta) {
		self.index += 1;
		debug_assert!(
			self.index < self.stack.len(),
			"NNUE accumulator stack overflow"
		);

		let entry = &mut self.stack[self.index];
		entry.accurate = false;
		entry.delta = delta;
	}

	fn pop(&mut self) {
		debug_assert!(self.index > 0);
		self.index -= 1;
	}

	fn evaluate(&mut self, board: &Board) -> i32 {
		let Some(params) = self.parameters.clone() else {
			return 0;
		};

		self.ensure_accurate(&params, board);

		let acc = &self.stack[self.index].accumulator;
		let output_bucket: usize = (board.occupancies().popcount() - 2) / 4;

		forward::output(acc, board.side_to_move(), &params, output_bucket)
	}

	/// `board` is the position at the current ply. If a king changed slot
	/// (crossed the d/e line or moved into another bucket) anywhere since the
	/// last accurate ply, replaying can't work (that perspective's features all
	/// changed), so this refreshes from `board` instead. The plies in between
	/// stay inaccurate and get rebuilt the same way if search comes back to them.
	fn ensure_accurate(&mut self, params: &Arc<NNUEParams<H>>, board: &Board) {
		if self.stack[self.index].accurate {
			return;
		}

		let mut last_accurate = self.index;
		while !self.stack[last_accurate].accurate {
			debug_assert!(last_accurate > 0, "no accurate accumulator found on stack");
			last_accurate -= 1;
		}

		if ((last_accurate + 1)..=self.index).any(|i| self.stack[i].delta.needs_refresh()) {
			self.full_refresh(board);
			return;
		}

		// No king changed slot, so the slots are the same all the way along
		let slots = self.stack[last_accurate].slots;
		for i in (last_accurate + 1)..=self.index {
			let (before, after) = self.stack.split_at_mut(i);
			let parent = &before[i - 1].accumulator;
			let child = &mut after[0];

			Self::apply_delta(parent, &mut child.accumulator, params, child.delta, slots);

			child.accurate = true;
			child.slots = slots;
		}
	}

	/// Replays one ply's feature changes, branching on move type exactly the
	/// way `Board::make_move` does - one fused `out = input + added - removed`
	/// pass per move instead of toggling features one by one.
	fn apply_delta(
		input: &Accumulator<H>,
		out: &mut Accumulator<H>,
		params: &NNUEParams<H>,
		delta: MoveDelta,
		slots: [KingSlot; 2],
	) {
		let mv = delta.mv;
		let piece = delta.piece;
		let from = mv.from();
		let to = mv.to();
		let stm = piece.color();

		if mv.is_castling() {
			let (rook_from, rook_to) = castling_rook_squares(stm, mv.flag());
			let rook = Piece::new(stm, PieceType::Rook);

			forward::add2_sub2(
				out,
				input,
				params,
				(piece, to),
				(rook, rook_to),
				(piece, from),
				(rook, rook_from),
				slots,
			);
		} else if mv.is_en_passant() {
			let cap_sq = to.shift(-UP_DIR[stm]);
			let captured_pawn = Piece::new(!stm, PieceType::Pawn);

			forward::add_sub2(
				out,
				input,
				params,
				(piece, to),
				(piece, from),
				(captured_pawn, cap_sq),
				slots,
			);
		} else {
			let final_piece = if mv.is_promotion() {
				Piece::new(stm, mv.promotion_piece_type())
			} else {
				piece
			};

			if mv.is_capture() {
				forward::add_sub2(
					out,
					input,
					params,
					(final_piece, to),
					(piece, from),
					(delta.captured, to),
					slots,
				);
			} else {
				forward::add_sub(out, input, params, (final_piece, to), (piece, from), slots);
			}
		}
	}
}

/// Owned by the search thread (put this on `SearchData`/`ThreadData`, not on
/// `Board`). `push`/`pop` are cheap - O(1), no accumulator math - and are
/// meant to be called in lockstep with `Board::make_move`/`undo_move`.
/// `evaluate()` is the only place accumulator work actually happens, and even
/// then only for the plies since the last accurate one. Holds both nets but
/// only keeps the active one's stack up to date.
pub struct NNUE {
	big: Network<BIG_HIDDEN>,
	small: Network<SMALL_HIDDEN>,
	use_small: bool,
}

impl NNUE {
	/// Snapshots whatever networks are currently loaded globally, and which one
	/// is active. Construct a fresh `NNUE` (or call `reload`) after loading a
	/// different network to pick it up.
	pub fn new() -> Self {
		Self {
			big: Network::new(BIG_PARAMS.read().unwrap().clone()),
			small: Network::new(SMALL_PARAMS.read().unwrap().clone()),
			use_small: active_net() == NetSize::Small,
		}
	}

	pub fn is_active(&self) -> bool {
		if self.use_small {
			self.small.parameters.is_some()
		} else {
			self.big.parameters.is_some()
		}
	}

	/// Resets the stack to a single, fully-refreshed root accumulator for
	/// `board`, with whichever net is active now (`set_net`). Call this whenever
	/// the position changes wholesale (new search, `position` command) rather
	/// than via incremental moves.
	pub fn reset(&mut self, board: &Board) {
		self.use_small = active_net() == NetSize::Small;
		if self.use_small {
			self.small.reset(board);
		} else {
			self.big.reset(board);
		}
	}

	/// Re-fetches the globally loaded networks (e.g. after `setoption EvalFile`)
	/// and rebuilds the accumulator for `board` from scratch.
	pub fn reload(&mut self, board: &Board) {
		self.big.parameters = BIG_PARAMS.read().unwrap().clone();
		self.small.parameters = SMALL_PARAMS.read().unwrap().clone();
		self.reset(board);
	}

	/// Records that `mv` is about to be played. Must be called with `board` in
	/// its *pre-move* state, immediately before `board.make_move(mv)`. Does no
	/// accumulator work - just stashes what's needed to replay it later.
	pub fn push(&mut self, mv: Move, board: &Board) {
		let delta = MoveDelta {
			mv,
			piece: board.get_piece_on_square(mv.from()),
			captured: board.get_piece_on_square(mv.to()),
		};
		if self.use_small {
			self.small.push(delta);
		} else {
			self.big.push(delta);
		}
	}

	/// Undoes the last `push`. Pair with `board.undo_move(mv)`.
	pub fn pop(&mut self) {
		if self.use_small {
			self.small.pop();
		} else {
			self.big.pop();
		}
	}

	/// Brings the accumulator at the current ply up to date (if needed) and
	/// evaluates the position. Side-to-move relative, same convention as
	/// `Board::evaluate`.
	pub fn evaluate(&mut self, board: &Board) -> i32 {
		if self.use_small {
			self.small.evaluate(board)
		} else {
			self.big.evaluate(board)
		}
	}
}

impl Default for NNUE {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::{
		Accumulator, BIG_PARAMS, DEFAULT_NET, KingSlot, MoveDelta, NNUE, NNUEParams, NetSize,
		Network, QA, QB, SCALE, SMALL_PARAMS, active_net, forward, king_slot, king_slots, set_net,
		try_init_embedded,
	};
	use crate::board::Board;
	use crate::board::generate_moves::AllMoves;
	use crate::types::{
		color::Color,
		moves::Move,
		piece::{Piece, PieceType},
		square::Square,
	};
	use std::sync::Mutex;

	/// The active net is a global; tests that change it take this so they don't race
	static NET_LOCK: Mutex<()> = Mutex::new(());

	const FENS: &[&str] = &[
		"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
		"r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
		"r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
		// kings on the d/e line and near bucket edges, with room to wander across them
		"8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
		"4k3/8/8/3pP3/8/8/8/3K4 w - d6 0 1",
		"8/3k4/8/8/8/8/4K3/8 w - - 0 1",
		"8/8/8/8/8/8/1k6/R3K3 b Q - 0 1",
	];

	fn next(seed: &mut u64) -> u64 {
		*seed ^= *seed << 13;
		*seed ^= *seed >> 7;
		*seed ^= *seed << 17;
		*seed
	}

	/// The lazily updated accumulator must match one built from scratch for the same board.
	fn assert_accurate<const H: usize>(net: &mut Network<H>, board: &Board) {
		let incremental = net.evaluate(board);
		let acc = net.stack[net.index].accumulator;

		let mut fresh = Network::new(net.parameters.clone());
		fresh.reset(board);
		let refreshed = fresh.evaluate(board);
		let fresh_acc = fresh.stack[0].accumulator;

		assert!(
			acc.white == fresh_acc.white && acc.black == fresh_acc.black,
			"accumulator differs, {board}"
		);
		assert_eq!(incremental, refreshed, "{board}");
	}

	/// Random walks shaped like search: go down a few plies, back up some, evaluating along the way (not at every
	/// ply, so the replay covers several plies and king slot changes in the middle of them).
	fn incremental_matches_refresh<const H: usize>(make: impl Fn() -> Network<H>) {
		let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
		let mut refreshes = 0;

		for fen in FENS {
			let root = Board::parse_fen_string(fen).unwrap();
			for _ in 0..30 {
				let mut board = root.clone();
				let mut net = make();
				net.reset(&board);
				let mut played: Vec<Move> = Vec::new();

				for _ in 0..120 {
					let moves = board.generate_all_legal_moves::<AllMoves>();
					let back = played.len() >= 6 || moves.is_empty() || next(&mut seed) % 4 == 0;
					if back && !played.is_empty() {
						let mv = played.pop().unwrap();
						board.undo_move(mv);
						net.pop();
					} else if !moves.is_empty() {
						let mv = moves.get(next(&mut seed) as usize % moves.len()).mv();
						let delta = MoveDelta {
							mv,
							piece: board.get_piece_on_square(mv.from()),
							captured: board.get_piece_on_square(mv.to()),
						};
						refreshes += delta.needs_refresh() as usize;
						net.push(delta);
						board.make_move(mv);
						played.push(mv);
					}
					if next(&mut seed) % 3 == 0 {
						assert_accurate(&mut net, &board);
					}
				}
			}
		}
		assert!(
			refreshes > 50,
			"only {refreshes} king slot changes, the test isn't covering refreshes"
		);
	}

	#[test]
	fn incremental_matches_refresh_big() {
		assert!(try_init_embedded());
		incremental_matches_refresh(|| Network::new(BIG_PARAMS.read().unwrap().clone()));
	}

	#[test]
	fn incremental_matches_refresh_small() {
		assert!(try_init_embedded());
		incremental_matches_refresh(|| Network::new(SMALL_PARAMS.read().unwrap().clone()));
	}

	/// Left-right mirror of a FEN, without castling rights (they don't mirror).
	fn mirror_fen(fen: &str) -> String {
		let parts: Vec<&str> = fen.split(' ').collect();
		let ranks: Vec<String> = parts[0]
			.split('/')
			.map(|r| r.chars().rev().collect())
			.collect();
		let ep = match parts[3] {
			"-" => "-".to_string(),
			sq => {
				let mut c = sq.chars();
				let file = (b'h' - (c.next().unwrap() as u8 - b'a')) as char;
				format!("{file}{}", c.as_str())
			}
		};
		format!(
			"{} {} - {} {} {}",
			ranks.join("/"),
			parts[1],
			ep,
			parts[4],
			parts[5]
		)
	}

	/// Mirrored inputs make the net blind to left vs right, so a position and its mirror image must evaluate the same,
	/// with both nets.
	#[test]
	fn mirror_image_evaluates_the_same() {
		let _lock = NET_LOCK.lock().unwrap();
		assert!(try_init_embedded());
		for net in [NetSize::Big, NetSize::Small] {
			set_net(net);
			for fen in FENS {
				let no_castling = {
					let p: Vec<&str> = fen.split(' ').collect();
					format!("{} {} - {} {} {}", p[0], p[1], p[3], p[4], p[5])
				};
				let a = Board::parse_fen_string(&no_castling).unwrap();
				let b = Board::parse_fen_string(&mirror_fen(fen)).unwrap();
				let (mut na, mut nb) = (NNUE::new(), NNUE::new());
				na.reset(&a);
				nb.reset(&b);
				assert_eq!(
					na.evaluate(&a),
					nb.evaluate(&b),
					"{net:?}: {no_castling} vs {}",
					mirror_fen(fen)
				);
			}
		}
		set_net(DEFAULT_NET);
	}

	/// `Net` really switches nets: the two give different evals for the same position.
	#[test]
	fn set_net_switches() {
		let _lock = NET_LOCK.lock().unwrap();
		assert!(try_init_embedded());
		let board = Board::parse_fen_string(FENS[1]).unwrap();
		let mut evals = Vec::new();
		for net in [NetSize::Big, NetSize::Small] {
			set_net(net);
			assert_eq!(active_net(), net);
			let mut nnue = NNUE::new();
			nnue.reset(&board);
			evals.push(nnue.evaluate(&board));
		}
		assert_ne!(evals[0], evals[1]);
		set_net(DEFAULT_NET);
	}

	/// Bullet's `ChessBucketsMirrored::new(KING_BUCKETS_4)`: each perspective flips by *its own* king's file and takes
	/// the bucket from its own king's square. (The symmetry test can't see this - any consistent left/right rule passes
	/// it - so pin the exact feature numbers.)
	#[test]
	fn each_perspective_follows_its_own_king() {
		// White king e1 (flipped, d1 = bucket 1), black king d8 (not flipped; d1 from its side = bucket 1)
		let board = Board::parse_fen_string("3k4/8/8/8/8/8/4P3/4K3 w - - 0 1").unwrap();
		let slots = king_slots(&board);
		assert_eq!(
			slots,
			[
				KingSlot {
					flip: true,
					offset: 768
				},
				KingSlot {
					flip: false,
					offset: 768
				}
			]
		);

		let e2 = Square::new(12);
		let white_pawn = Piece::new(Color::White, PieceType::Pawn);
		// White's view: own pawn (0..63), e2 flipped to d2 = 11. Black's view: an enemy pawn (384..447), e2 seen from
		// black's side is e7 = 52, not flipped. Both in bucket 1's block.
		assert_eq!(
			forward::indices(white_pawn, e2, slots),
			(768 + 11, 768 + 384 + 52)
		);

		// Buckets: g1 -> b1 = 0, c1 = 1, e2 -> d2 = 2, anything from rank 3 up = 3; black's from its own side
		let bucket = |c, sq: u8| king_slot(c, Square::new(sq)).offset / 768;
		assert_eq!(bucket(Color::White, 6), 0); // g1
		assert_eq!(bucket(Color::White, 2), 1); // c1
		assert_eq!(bucket(Color::White, 12), 2); // e2
		assert_eq!(bucket(Color::White, 20), 3); // e3
		assert_eq!(bucket(Color::White, 62), 3); // g8
		assert_eq!(bucket(Color::Black, 62), 0); // g8 = g1 from black's side
		assert_eq!(bucket(Color::Black, 52), 2); // e7 = e2 from black's side
		assert_eq!(bucket(Color::Black, 6), 3); // g1 = g8 from black's side
	}

	/// The (possibly SIMD) output matches a plain wide-integer version of the same formula.
	#[test]
	fn output_matches_reference() {
		fn reference<const H: usize>(
			acc: &Accumulator<H>,
			stm: Color,
			p: &NNUEParams<H>,
			bucket: usize,
		) -> i32 {
			let (us, them) = match stm {
				Color::White => (&acc.white, &acc.black),
				Color::Black => (&acc.black, &acc.white),
			};
			let mut sum: i64 = 0;
			for i in 0..H {
				let u = (us[i] as i64).clamp(0, QA as i64);
				let t = (them[i] as i64).clamp(0, QA as i64);
				sum += u * u * p.output_weights[bucket][0][i] as i64;
				sum += t * t * p.output_weights[bucket][1][i] as i64;
			}
			(((sum / QA as i64) as i32 + p.output_bias[bucket] as i32) * SCALE) / (QA * QB)
		}
		fn check<const H: usize>(mut net: Network<H>) {
			for fen in FENS {
				let board = Board::parse_fen_string(fen).unwrap();
				net.reset(&board);
				let params = net.parameters.clone().unwrap();
				let acc = net.stack[0].accumulator;
				let bucket = (board.occupancies().popcount() - 2) / 4;
				for stm in [Color::White, Color::Black] {
					assert_eq!(
						forward::output(&acc, stm, &params, bucket),
						reference(&acc, stm, &params, bucket),
						"{fen}"
					);
				}
			}
		}
		assert!(try_init_embedded());
		check(Network::new(BIG_PARAMS.read().unwrap().clone()));
		check(Network::new(SMALL_PARAMS.read().unwrap().clone()));
	}
}
