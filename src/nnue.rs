// Architecture ported from Reckless: the accumulator lives on the search thread
// (as a `Network`), not on `Board`. `push`/`pop` just record *what* is about to
// change; the actual feature add/remove math only happens lazily inside
// `evaluate()`, which walks back to the last accurate ply on the stack and
// replays deltas forward. Internal search nodes that never call `evaluate()`
// (i.e. every non-leaf node) pay zero NNUE cost.
//
// Topology: single (768-128)x2-1 SCReLU net with horizontally mirrored inputs,
// no king buckets, no threat inputs, no L2/L3. Each perspective whose king is on
// files e-h sees the board flipped left-to-right (`square ^ 7`), the same as
// bullet's `ChessBucketsMirrored`. A king crossing between the d and e files
// changes every feature of its perspective, so that's the one "can't update,
// must refresh" case: `ensure_accurate` refreshes from the board instead of
// replaying.

use std::mem::transmute;
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
pub const HIDDEN_SIZE: usize = 128;
pub const QA: i32 = 255; // don't go above 255 (i16 squared must stay < 32768 headroom-safe)
pub const QB: i32 = 64;
pub const SCALE: i32 = 400;

const _: () = assert!(HIDDEN_SIZE % 16 == 0);

// =====================================================================================================================
// Network parameters (weights/biases) - global, reference-counted, swappable at runtime
// =====================================================================================================================

/* `repr(C)` pins the field order/padding to match the raw file layout exactly
(inputWeights, then inputBiases, then outputWeights, then outputBias, all
i16, no gaps) - required for `embedded()`'s `transmute` below to be valid. */
#[repr(C)]
#[derive(Clone)]
#[repr(align(64))]
pub struct NNUEParams {
	// input x hidden
	input_weights: [[i16; HIDDEN_SIZE]; INPUT_SIZE],
	input_biases: [i16; HIDDEN_SIZE],
	// 2 perspectives
	output_weights: [i16; HIDDEN_SIZE * 2],
	output_bias: i16,
}

/// Place a trained network at this path (relative to this file) and add
/// `embed-nnue` to the crate's default features (or pass --features embed-nnue)
/// to bake it into the binary
#[cfg(feature = "embed-nnue")]
const EXPECTED: usize = std::mem::size_of::<NNUEParams>();
#[cfg(feature = "embed-nnue")]

const ACTUAL: usize = include_bytes!("nnue/768hm-128x2-1.bin").len();
#[cfg(feature = "embed-nnue")]

const _: [(); EXPECTED] = [(); ACTUAL];

impl NNUEParams {
	/// Reinterprets the network baked in at compile time directly as `Self` -
	/// no parsing loop, no heap allocation, the parameters just live in the
	/// binary's data section. Relies on `#[repr(C)]` above and the size
	/// assertion above to make the `transmute` valid; assumes a
	/// little-endian build target, same as the old C++
	/// `loadFromPtr`/`memcpy` did implicitly.
	#[cfg(feature = "embed-nnue")]
	fn embedded() -> &'static Self {
		static EMBEDDED: NNUEParams = unsafe { transmute(*include_bytes!("nnue/768hm-128x2-1.bin")) };
		&EMBEDDED
	}
}

static NNUE_PARAMS: RwLock<Option<Arc<NNUEParams>>> = RwLock::new(None);

fn current_params() -> Option<Arc<NNUEParams>> {
	NNUE_PARAMS.read().unwrap().clone()
}

/// Whether a network is currently loaded globally. Individual `Network`
/// instances snapshot this at construction/reset time (see `Network::new`),
/// so this is mainly useful for startup logging.
pub fn is_loaded() -> bool {
	NNUE_PARAMS.read().unwrap().is_some()
}

/// Copies a raw network file straight into a fresh `NNUEParams`, same layout
/// `embedded()` and the old C++ `loadFromPtr` both assume (inputWeights, then
/// inputBiases, then outputWeights, then outputBias, `#[repr(C)]`,
/// little-endian). Bounds-checked up front, unlike the original `memcpy`
/// chain, but otherwise a straight byte copy - no per-element parsing loop.
fn load_from_bytes(bytes: &[u8]) -> Result<(), String> {
	let expected_len = std::mem::size_of::<NNUEParams>();
	if bytes.len() != expected_len {
		return Err(format!(
			"NNUE file wrong size: got {} bytes, expected exactly {expected_len}",
			bytes.len()
		));
	}

	// SAFETY: `NNUEParams` is `#[repr(C)]` and made up entirely of `i16`
	// arrays (no padding, no niches), so overwriting a zeroed instance with
	// exactly `size_of::<NNUEParams>()` bytes is well-defined.
	let mut params: Box<NNUEParams> = Box::new(unsafe { std::mem::zeroed() });
	unsafe {
		std::ptr::copy_nonoverlapping(
			bytes.as_ptr(),
			params.as_mut() as *mut NNUEParams as *mut u8,
			expected_len,
		);
	}

	*NNUE_PARAMS.write().unwrap() = Some(Arc::from(params));
	Ok(())
}

/// Mirrors `initNNUEEmbedded()` - loads whatever network was baked in at compile
/// time via the `embed-nnue` feature. Returns false (and loads nothing) if that
/// feature isn't enabled, same as "no embedded network found".
pub fn try_init_embedded() -> bool {
	#[cfg(feature = "embed-nnue")]
	{
		*NNUE_PARAMS.write().unwrap() = Some(Arc::new(NNUEParams::embedded().clone()));
		true
	}
	#[cfg(not(feature = "embed-nnue"))]
	{
		false
	}
}

/// Mirrors `initNNUE(filename)` - loads a network from disk at runtime,
/// e.g. via `setoption name EvalFile value <path>`. Every `Network` created
/// after this call (i.e. every subsequent search) will pick it up.
pub fn init_from_file(path: &str) -> Result<(), String> {
	let bytes =
		std::fs::read(path).map_err(|e| format!("Failed to open NNUE file '{path}': {e}"))?;
	load_from_bytes(&bytes)
}

// =====================================================================================================================
// Accumulator
// =====================================================================================================================

#[derive(Clone, Copy)]
#[repr(align(64))]
struct Accumulator {
	white: [i16; HIDDEN_SIZE],
	black: [i16; HIDDEN_SIZE],
}

impl Default for Accumulator {
	fn default() -> Self {
		Self {
			white: [0; HIDDEN_SIZE],
			black: [0; HIDDEN_SIZE],
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
	use super::{Accumulator, HIDDEN_SIZE, NNUEParams, QA, QB, SCALE, feature_index};
	use crate::types::{color::Color, piece::Piece, square::Square};

	/// `idx` for white's perspective, `midx` for black's (colours swapped and the
	/// board flipped top-to-bottom), each flipped left-to-right when that
	/// perspective's king is on files e-h (`flip`, indexed by colour).
	pub(super) fn indices(piece: Piece, square: Square, flip: [bool; 2]) -> (usize, usize) {
		let white_sq = if flip[0] { square.flip_file() } else { square };
		let black_sq = if flip[1] { square.flip_rank().flip_file() } else { square.flip_rank() };
		let mirrored_piece = Piece::new(!piece.color(), piece.piece_type());
		(feature_index(piece, white_sq), feature_index(mirrored_piece, black_sq))
	}

	pub fn add_feature(acc: &mut Accumulator, params: &NNUEParams, piece: Piece, square: Square, flip: [bool; 2]) {
		let (idx, midx) = indices(piece, square, flip);

		#[cfg(target_feature = "avx2")]
		unsafe {
			avx2::add(acc, params, idx, midx);
		}
		#[cfg(not(target_feature = "avx2"))]
		scalar::add(acc, params, idx, midx);
	}

	// ---- fused updates: `out = input + added - removed`, in one pass over the accumulator ----

	/// Quiet moves and quiet promotions: one feature added, one removed
	pub fn add_sub(
		out: &mut Accumulator,
		input: &Accumulator,
		params: &NNUEParams,
		add: (Piece, Square),
		sub: (Piece, Square),
		flip: [bool; 2],
	) {
		let add = indices(add.0, add.1, flip);
		let sub = indices(sub.0, sub.1, flip);

		#[cfg(target_feature = "avx2")]
		unsafe {
			avx2::add_sub(out, input, params, add, sub);
		}
		#[cfg(not(target_feature = "avx2"))]
		scalar::add_sub(out, input, params, add, sub);
	}

	/// Captures, en passant and capture-promotions: one added, two removed
	pub fn add_sub2(
		out: &mut Accumulator,
		input: &Accumulator,
		params: &NNUEParams,
		add: (Piece, Square),
		sub1: (Piece, Square),
		sub2: (Piece, Square),
		flip: [bool; 2],
	) {
		let add = indices(add.0, add.1, flip);
		let sub1 = indices(sub1.0, sub1.1, flip);
		let sub2 = indices(sub2.0, sub2.1, flip);

		#[cfg(target_feature = "avx2")]
		unsafe {
			avx2::add_sub2(out, input, params, add, sub1, sub2);
		}
		#[cfg(not(target_feature = "avx2"))]
		scalar::add_sub2(out, input, params, add, sub1, sub2);
	}

	/// Castling: king and rook both move, two added, two removed
	pub fn add2_sub2(
		out: &mut Accumulator,
		input: &Accumulator,
		params: &NNUEParams,
		add1: (Piece, Square),
		add2: (Piece, Square),
		sub1: (Piece, Square),
		sub2: (Piece, Square),
		flip: [bool; 2],
	) {
		let add1 = indices(add1.0, add1.1, flip);
		let add2 = indices(add2.0, add2.1, flip);
		let sub1 = indices(sub1.0, sub1.1, flip);
		let sub2 = indices(sub2.0, sub2.1, flip);

		#[cfg(target_feature = "avx2")]
		unsafe {
			avx2::add2_sub2(out, input, params, add1, add2, sub1, sub2);
		}
		#[cfg(not(target_feature = "avx2"))]
		scalar::add2_sub2(out, input, params, add1, add2, sub1, sub2);
	}

	pub fn output(acc: &Accumulator, stm: Color, params: &NNUEParams) -> i32 {
		#[cfg(target_feature = "avx2")]
		unsafe {
			avx2::output(acc, stm, params)
		}
		#[cfg(not(target_feature = "avx2"))]
		scalar::output(acc, stm, params)
	}

	// ---- scalar fallback: portable, relies on LLVM auto-vectorization ----
	#[cfg(not(target_feature = "avx2"))]
	mod scalar {
		use super::{Accumulator, HIDDEN_SIZE, NNUEParams, QA, QB, SCALE};
		use crate::types::color::Color;

		pub fn add(acc: &mut Accumulator, params: &NNUEParams, idx: usize, midx: usize) {
			let weights = &params.input_weights[idx];
			for i in 0..HIDDEN_SIZE {
				acc.white[i] += weights[i];
			}

			let mirrored_weights = &params.input_weights[midx];
			for i in 0..HIDDEN_SIZE {
				acc.black[i] += mirrored_weights[i];
			}
		}

		// Fused updates. Wrapping math like the SIMD versions: an intermediate sum may overflow i16,
		// the final value is still right.
		pub fn add_sub(
			out: &mut Accumulator,
			input: &Accumulator,
			params: &NNUEParams,
			add: (usize, usize),
			sub: (usize, usize),
		) {
			let w = &params.input_weights;
			for i in 0..HIDDEN_SIZE {
				out.white[i] = input.white[i]
					.wrapping_add(w[add.0][i])
					.wrapping_sub(w[sub.0][i]);
			}
			for i in 0..HIDDEN_SIZE {
				out.black[i] = input.black[i]
					.wrapping_add(w[add.1][i])
					.wrapping_sub(w[sub.1][i]);
			}
		}

		pub fn add_sub2(
			out: &mut Accumulator,
			input: &Accumulator,
			params: &NNUEParams,
			add: (usize, usize),
			sub1: (usize, usize),
			sub2: (usize, usize),
		) {
			let w = &params.input_weights;
			for i in 0..HIDDEN_SIZE {
				out.white[i] = input.white[i]
					.wrapping_add(w[add.0][i])
					.wrapping_sub(w[sub1.0][i])
					.wrapping_sub(w[sub2.0][i]);
			}
			for i in 0..HIDDEN_SIZE {
				out.black[i] = input.black[i]
					.wrapping_add(w[add.1][i])
					.wrapping_sub(w[sub1.1][i])
					.wrapping_sub(w[sub2.1][i]);
			}
		}

		pub fn add2_sub2(
			out: &mut Accumulator,
			input: &Accumulator,
			params: &NNUEParams,
			add1: (usize, usize),
			add2: (usize, usize),
			sub1: (usize, usize),
			sub2: (usize, usize),
		) {
			let w = &params.input_weights;
			for i in 0..HIDDEN_SIZE {
				out.white[i] = input.white[i]
					.wrapping_add(w[add1.0][i])
					.wrapping_add(w[add2.0][i])
					.wrapping_sub(w[sub1.0][i])
					.wrapping_sub(w[sub2.0][i]);
			}
			for i in 0..HIDDEN_SIZE {
				out.black[i] = input.black[i]
					.wrapping_add(w[add1.1][i])
					.wrapping_add(w[add2.1][i])
					.wrapping_sub(w[sub1.1][i])
					.wrapping_sub(w[sub2.1][i]);
			}
		}

		pub fn output(acc: &Accumulator, stm: Color, params: &NNUEParams) -> i32 {
			let (us, them) = match stm {
				Color::White => (&acc.white, &acc.black),
				Color::Black => (&acc.black, &acc.white),
			};

			let w0 = &params.output_weights[0..HIDDEN_SIZE];
			let w1 = &params.output_weights[HIDDEN_SIZE..HIDDEN_SIZE * 2];

			let mut sum: i32 = 0;
			for i in 0..HIDDEN_SIZE {
				let u = (us[i] as i32).clamp(0, QA);
				sum += (u * u) * (w0[i] as i32);

				let t = (them[i] as i32).clamp(0, QA);
				sum += (t * t) * (w1[i] as i32);
			}

			let mut output = sum / QA;
			output += params.output_bias as i32;
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
		use super::{Accumulator, HIDDEN_SIZE, NNUEParams, QA, QB, SCALE};
		use crate::types::color::Color;
		use std::arch::x86_64::*;

		#[target_feature(enable = "avx2")]
		unsafe fn add_half(half: &mut [i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) {
			unsafe {
				for i in (0..HIDDEN_SIZE).step_by(16) {
					let acc = _mm256_loadu_si256(half.as_ptr().add(i) as *const __m256i);
					let w = _mm256_loadu_si256(weights.as_ptr().add(i) as *const __m256i);
					let sum = _mm256_add_epi16(acc, w);
					_mm256_storeu_si256(half.as_mut_ptr().add(i) as *mut __m256i, sum);
				}
			}
		}

		#[target_feature(enable = "avx2")]
		unsafe fn add_sub_half(
			out: &mut [i16; HIDDEN_SIZE],
			input: &[i16; HIDDEN_SIZE],
			w_add: &[i16; HIDDEN_SIZE],
			w_sub: &[i16; HIDDEN_SIZE],
		) {
			unsafe {
				for i in (0..HIDDEN_SIZE).step_by(16) {
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
		unsafe fn add_sub2_half(
			out: &mut [i16; HIDDEN_SIZE],
			input: &[i16; HIDDEN_SIZE],
			w_add: &[i16; HIDDEN_SIZE],
			w_sub1: &[i16; HIDDEN_SIZE],
			w_sub2: &[i16; HIDDEN_SIZE],
		) {
			unsafe {
				for i in (0..HIDDEN_SIZE).step_by(16) {
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
		unsafe fn add2_sub2_half(
			out: &mut [i16; HIDDEN_SIZE],
			input: &[i16; HIDDEN_SIZE],
			w_add1: &[i16; HIDDEN_SIZE],
			w_add2: &[i16; HIDDEN_SIZE],
			w_sub1: &[i16; HIDDEN_SIZE],
			w_sub2: &[i16; HIDDEN_SIZE],
		) {
			unsafe {
				for i in (0..HIDDEN_SIZE).step_by(16) {
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

		pub unsafe fn add(acc: &mut Accumulator, params: &NNUEParams, idx: usize, midx: usize) {
			unsafe {
				add_half(&mut acc.white, &params.input_weights[idx]);
				add_half(&mut acc.black, &params.input_weights[midx]);
			}
		}

		// Fused wrappers: each feature is (white index, black index), see `forward::indices`
		pub unsafe fn add_sub(
			out: &mut Accumulator,
			input: &Accumulator,
			params: &NNUEParams,
			add: (usize, usize),
			sub: (usize, usize),
		) {
			let w = &params.input_weights;
			unsafe {
				add_sub_half(&mut out.white, &input.white, &w[add.0], &w[sub.0]);
				add_sub_half(&mut out.black, &input.black, &w[add.1], &w[sub.1]);
			}
		}

		pub unsafe fn add_sub2(
			out: &mut Accumulator,
			input: &Accumulator,
			params: &NNUEParams,
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

		pub unsafe fn add2_sub2(
			out: &mut Accumulator,
			input: &Accumulator,
			params: &NNUEParams,
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

		/// Squared-clamped-ReLU dot product - same lane layout as the original
		/// AVX2 `evaluateNNUE`: 16 x i16 per iteration, widened to i32 pairs for
		/// the multiply-accumulate, horizontally reduced at the end.
		#[target_feature(enable = "avx2")]
		pub unsafe fn output(acc: &Accumulator, stm: Color, params: &NNUEParams) -> i32 {
			let (us, them) = match stm {
				Color::White => (&acc.white, &acc.black),
				Color::Black => (&acc.black, &acc.white),
			};

			let w0 = &params.output_weights[0..HIDDEN_SIZE];
			let w1 = &params.output_weights[HIDDEN_SIZE..HIDDEN_SIZE * 2];

			let zero = _mm256_setzero_si256();
			let qa = _mm256_set1_epi16(QA as i16);
			let mut sum = _mm256_setzero_si256();

			unsafe {
				for i in (0..HIDDEN_SIZE).step_by(16) {
					// ── "us" half ──────────────────────────────────────────────
					let mut u = _mm256_loadu_si256(us.as_ptr().add(i) as *const __m256i);
					let wu = _mm256_loadu_si256(w0.as_ptr().add(i) as *const __m256i);

					u = _mm256_max_epi16(u, zero);
					u = _mm256_min_epi16(u, qa);

					let u_sq = _mm256_mullo_epi16(u, u);
					let u_sq_lo = _mm256_cvtepu16_epi32(_mm256_castsi256_si128(u_sq));
					let u_sq_hi = _mm256_cvtepu16_epi32(_mm256_extracti128_si256(u_sq, 1));
					let wu_lo = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(wu));
					let wu_hi = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(wu, 1));

					sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(u_sq_lo, wu_lo));
					sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(u_sq_hi, wu_hi));

					// ── "them" half ────────────────────────────────────────────
					let mut t = _mm256_loadu_si256(them.as_ptr().add(i) as *const __m256i);
					let wt = _mm256_loadu_si256(w1.as_ptr().add(i) as *const __m256i);

					t = _mm256_max_epi16(t, zero);
					t = _mm256_min_epi16(t, qa);

					let t_sq = _mm256_mullo_epi16(t, t);
					let t_sq_lo = _mm256_cvtepu16_epi32(_mm256_castsi256_si128(t_sq));
					let t_sq_hi = _mm256_cvtepu16_epi32(_mm256_extracti128_si256(t_sq, 1));
					let wt_lo = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(wt));
					let wt_hi = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(wt, 1));

					sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(t_sq_lo, wt_lo));
					sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(t_sq_hi, wt_hi));
				}
			}

			// Horizontal reduction: 8 x i32 lanes -> scalar
			let lo = _mm256_castsi256_si128(sum);
			let hi = _mm256_extracti128_si256(sum, 1);
			let mut s = _mm_add_epi32(lo, hi);
			s = _mm_add_epi32(s, _mm_srli_si128(s, 8));
			s = _mm_add_epi32(s, _mm_srli_si128(s, 4));
			let mut output = _mm_cvtsi128_si32(s);

			output /= QA;
			output += params.output_bias as i32;
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
	/// A king moving between the a-d and e-h halves flips its perspective's
	/// mirroring, which changes every feature of that perspective.
	fn crosses_mirror(self) -> bool {
		self.piece.piece_type() == PieceType::King
			&& (self.mv.from().get_file() as u8 >= 4) != (self.mv.to().get_file() as u8 >= 4)
	}
}

#[derive(Clone, Copy, Default)]
struct StackEntry {
	accumulator: Accumulator,
	accurate: bool,
	delta: MoveDelta,
	/// Per colour: that perspective's king is on files e-h, so its features are
	/// flipped left-to-right. Only meaningful while `accurate`.
	flip: [bool; 2],
}

/// Whether each perspective (indexed by colour) is mirrored in `board`.
fn mirror_flags(board: &Board) -> [bool; 2] {
	[
		board.king_square(Color::White).get_file() as u8 >= 4,
		board.king_square(Color::Black).get_file() as u8 >= 4,
	]
}

/// Owned by the search thread (put this on `SearchData`/`ThreadData`, not on
/// `Board`). `push`/`pop` are cheap - O(1), no accumulator math - and are
/// meant to be called in lockstep with `Board::make_move`/`undo_move`.
/// `evaluate()` is the only place accumulator work actually happens, and even
/// then only for the plies since the last accurate one.
pub struct NNUE {
	parameters: Option<Arc<NNUEParams>>,
	stack: Box<[StackEntry]>,
	index: usize,
}

impl NNUE {
	/// Snapshots whatever network is currently loaded globally. Construct a
	/// fresh `Network` (or call `reset`) after loading a different network to
	/// pick it up.
	pub fn new() -> Self {
		Self {
			parameters: current_params(),
			stack: vec![StackEntry::default(); MAX_PLY + 1].into_boxed_slice(),
			index: 0,
		}
	}

	pub fn is_active(&self) -> bool {
		self.parameters.is_some()
	}

	/// Resets the stack to a single, fully-refreshed root accumulator for
	/// `board`. Call this whenever the position changes wholesale (new search,
	/// `position` command, after loading a different network) rather than via
	/// incremental moves.
	pub fn reset(&mut self, board: &Board) {
		self.index = 0;
		self.full_refresh(board);
	}

	/// Re-fetches the globally loaded network (e.g. after `setoption EvalFile`)
	/// and rebuilds the accumulator for `board` from scratch.
	pub fn reload(&mut self, board: &Board) {
		self.parameters = current_params();
		self.reset(board);
	}

	fn full_refresh(&mut self, board: &Board) {
		let Some(params) = self.parameters.clone() else {
			self.stack[self.index].accurate = true;
			return;
		};

		let mut acc = Accumulator {
			white: params.input_biases,
			black: params.input_biases,
		};
		let flip = mirror_flags(board);

		for i in 0..Square::NUM as u8 {
			let square = Square::new(i);
			let piece = board.get_piece_on_square(square);
			if piece != Piece::None {
				forward::add_feature(&mut acc, &params, piece, square, flip);
			}
		}

		self.stack[self.index].accumulator = acc;
		self.stack[self.index].accurate = true;
		self.stack[self.index].flip = flip;
	}

	/// Records that `mv` is about to be played. Must be called with `board` in
	/// its *pre-move* state, immediately before `board.make_move(mv)`. Does no
	/// accumulator work - just stashes what's needed to replay it later.
	pub fn push(&mut self, mv: Move, board: &Board) {
		self.index += 1;
		debug_assert!(
			self.index < self.stack.len(),
			"NNUE accumulator stack overflow"
		);

		let entry = &mut self.stack[self.index];
		entry.accurate = false;
		entry.delta = MoveDelta {
			mv,
			piece: board.get_piece_on_square(mv.from()),
			captured: board.get_piece_on_square(mv.to()),
		};
	}

	/// Undoes the last `push`. Pair with `board.undo_move(mv)`.
	pub fn pop(&mut self) {
		debug_assert!(self.index > 0);
		self.index -= 1;
	}

	/// Brings the accumulator at the current ply up to date (if needed) and
	/// evaluates the position. Side-to-move relative, same convention as
	/// `Board::evaluate`.
	pub fn evaluate(&mut self, board: &Board) -> i32 {
		let Some(params) = self.parameters.clone() else {
			return 0;
		};

		self.ensure_accurate(&params, board);

		let acc = &self.stack[self.index].accumulator;
		forward::output(acc, board.side_to_move(), &params)
	}

	/// `board` is the position at the current ply. If a king crossed the d/e
	/// line anywhere since the last accurate ply, replaying can't work (that
	/// perspective's features all changed), so this refreshes from `board`
	/// instead. The plies in between stay inaccurate and get rebuilt the same
	/// way if search comes back to them.
	fn ensure_accurate(&mut self, params: &Arc<NNUEParams>, board: &Board) {
		if self.stack[self.index].accurate {
			return;
		}

		let mut last_accurate = self.index;
		while !self.stack[last_accurate].accurate {
			debug_assert!(last_accurate > 0, "no accurate accumulator found on stack");
			last_accurate -= 1;
		}

		if ((last_accurate + 1)..=self.index).any(|i| self.stack[i].delta.crosses_mirror()) {
			self.full_refresh(board);
			return;
		}

		// No crossing, so the mirroring is the same all the way along
		let flip = self.stack[last_accurate].flip;
		for i in (last_accurate + 1)..=self.index {
			let (before, after) = self.stack.split_at_mut(i);
			let parent = &before[i - 1].accumulator;
			let child = &mut after[0];

			Self::apply_delta(parent, &mut child.accumulator, params, child.delta, flip);

			child.accurate = true;
			child.flip = flip;
		}
	}

	/// Replays one ply's feature changes, branching on move type exactly the
	/// way `Board::make_move` does - one fused `out = input + added - removed`
	/// pass per move instead of toggling features one by one.
	fn apply_delta(
		input: &Accumulator,
		out: &mut Accumulator,
		params: &NNUEParams,
		delta: MoveDelta,
		flip: [bool; 2],
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
				flip,
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
				flip,
			);
		} else {
			let final_piece = if mv.is_promotion() {
				Piece::new(stm, mv.promotion_piece_type())
			} else {
				piece
			};

			if mv.is_capture() {
				forward::add_sub2(out, input, params, (final_piece, to), (piece, from), (delta.captured, to), flip);
			} else {
				forward::add_sub(out, input, params, (final_piece, to), (piece, from), flip);
			}
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
	use super::{NNUE, forward, mirror_flags, try_init_embedded};
	use crate::board::Board;
	use crate::board::generate_moves::AllMoves;
	use crate::types::{
		color::Color,
		moves::Move,
		piece::{Piece, PieceType},
		square::Square,
	};

	const FENS: &[&str] = &[
		"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
		"r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
		"r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
		// kings on the d/e line with room to wander across it
		"8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
		"4k3/8/8/3pP3/8/8/8/3K4 w - d6 0 1",
		"8/3k4/8/8/8/8/4K3/8 w - - 0 1",
	];

	fn next(seed: &mut u64) -> u64 {
		*seed ^= *seed << 13;
		*seed ^= *seed >> 7;
		*seed ^= *seed << 17;
		*seed
	}

	/// The lazily updated accumulator must match one built from scratch for the same board.
	fn assert_accurate(nnue: &mut NNUE, board: &Board) {
		let incremental = nnue.evaluate(board);
		let acc = nnue.stack[nnue.index].accumulator;

		let mut fresh = NNUE::new();
		fresh.reset(board);
		let refreshed = fresh.evaluate(board);
		let fresh_acc = fresh.stack[0].accumulator;

		assert!(acc.white == fresh_acc.white && acc.black == fresh_acc.black, "accumulator differs, {board}");
		assert_eq!(incremental, refreshed, "{board}");
	}

	/// Random walks shaped like search: go down a few plies, back up some, evaluating along the way (not at every
	/// ply, so the replay covers several plies and king crossings in the middle of them).
	#[test]
	fn incremental_matches_refresh() {
		assert!(try_init_embedded());
		let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
		let mut crossings = 0;

		for fen in FENS {
			let root = Board::parse_fen_string(fen).unwrap();
			for _ in 0..30 {
				let mut board = root.clone();
				let mut nnue = NNUE::new();
				nnue.reset(&board);
				let mut played: Vec<Move> = Vec::new();

				for _ in 0..120 {
					let moves = board.generate_all_legal_moves::<AllMoves>();
					let back = played.len() >= 6 || moves.is_empty() || next(&mut seed) % 4 == 0;
					if back && !played.is_empty() {
						let mv = played.pop().unwrap();
						board.undo_move(mv);
						nnue.pop();
					} else if !moves.is_empty() {
						let mv = moves.get(next(&mut seed) as usize % moves.len()).mv();
						nnue.push(mv, &board);
						crossings += nnue.stack[nnue.index].delta.crosses_mirror() as usize;
						board.make_move(mv);
						played.push(mv);
					}
					if next(&mut seed) % 3 == 0 {
						assert_accurate(&mut nnue, &board);
					}
				}
			}
		}
		assert!(crossings > 50, "only {crossings} king crossings, the test isn't covering refreshes");
	}

	/// Left-right mirror of a FEN, without castling rights (they don't mirror).
	fn mirror_fen(fen: &str) -> String {
		let parts: Vec<&str> = fen.split(' ').collect();
		let ranks: Vec<String> = parts[0].split('/').map(|r| r.chars().rev().collect()).collect();
		let ep = match parts[3] {
			"-" => "-".to_string(),
			sq => {
				let mut c = sq.chars();
				let file = (b'h' - (c.next().unwrap() as u8 - b'a')) as char;
				format!("{file}{}", c.as_str())
			}
		};
		format!("{} {} - {} {} {}", ranks.join("/"), parts[1], ep, parts[4], parts[5])
	}

	/// Mirrored inputs make the net blind to left vs right, so a position and its mirror image must evaluate the same.
	#[test]
	fn mirror_image_evaluates_the_same() {
		assert!(try_init_embedded());
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
			assert_eq!(na.evaluate(&a), nb.evaluate(&b), "{no_castling} vs {}", mirror_fen(fen));
		}
	}

	/// Bullet's `ChessBucketsMirrored`: each perspective flips by *its own* king's file. (The symmetry test can't see
	/// this - any consistent left/right rule passes it - so pin the exact feature numbers.)
	#[test]
	fn each_perspective_follows_its_own_king() {
		// White king e1 (flipped), black king d8 (not)
		let board = Board::parse_fen_string("3k4/8/8/8/8/8/4P3/4K3 w - - 0 1").unwrap();
		assert_eq!(mirror_flags(&board), [true, false]);

		let e2 = Square::new(12);
		let white_pawn = Piece::new(Color::White, PieceType::Pawn);
		// White's view: own pawn (0..63), e2 flipped to d2 = 11. Black's view: an enemy pawn (384..447), e2 seen from
		// black's side is e7 = 52, not flipped.
		assert_eq!(forward::indices(white_pawn, e2, [true, false]), (11, 384 + 52));
		// And the other way round: only black's view flips, e7 -> d7 = 51
		assert_eq!(forward::indices(white_pawn, e2, [false, true]), (12, 384 + 51));
	}
}
