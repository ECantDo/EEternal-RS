use std::sync::{Arc, RwLock};

use crate::{
    board::Board,
    types::{
        castling::castling_rook_squares, moves::Move,
        piece::{Piece, PieceType},
        square::Square,
        MAX_PLY,
        UP_DIR,
    },
};

pub const INPUT_SIZE: usize = 768;
pub const HIDDEN_SIZE: usize = 128;
pub const QA: i32 = 255; // don't go above 255 (i16 squared must stay < 32768 headroom-safe)
pub const QB: i32 = 64;
pub const SCALE: i32 = 400;

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
    pub input_weights: [[i16; HIDDEN_SIZE]; INPUT_SIZE],
    pub input_biases: [i16; HIDDEN_SIZE],
    // 2 perspectives
    pub output_weights: [i16; HIDDEN_SIZE * 2],
    pub output_bias: i16,
}

/// Place a trained network at this path (relative to this file) and add
/// `embed-nnue` to the crate's default features (or pass --features embed-nnue)
/// to bake it into the binary
#[cfg(feature = "embed-nnue")]
const EXPECTED: usize = std::mem::size_of::<NNUEParams>();
#[cfg(feature = "embed-nnue")]

const NNUE_BYTES: &[u8] = include_bytes!("nnue/768-128x2-1.bin");

const ACTUAL: usize = NNUE_BYTES.len();

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
    pub fn load_embedded() -> Self {
        assert_eq!(
            NNUE_BYTES.len(),
            size_of::<NNUEParams>(),
            "embedded NNUE file size doesn't match NNUEParams layout"
        );
        // read_unaligned: include_bytes! doesn't guarantee i16 alignment,
        // this avoids UB from reading through a misaligned pointer.
        unsafe { std::ptr::read_unaligned(NNUE_BYTES.as_ptr() as *const NNUEParams) }
    }
}

// =====================================================================================================================
// Accumulator
// =====================================================================================================================

#[derive(Clone, Copy)]
#[repr(align(64), C)]
pub struct Accumulator {
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

impl Accumulator {
    pub fn from_biases(params: &NNUEParams) -> Self {
        Self { white: params.input_biases, black: params.input_biases }
    }

    /// Full rebuild from scratch. Call when the position changes by any
    /// means other than search's own make/undo (e.g. UCI `position`).
    pub fn reset(&mut self, board: &Board, params: &NNUEParams) {
        *self = Self::from_biases(params);
        for sq in 0..64u8 {
            let square = Square::new(sq);
            let piece = board.get_piece_on_square(square);
            if piece != Piece::None {
                add_at(self, params, piece, square);
            }
        }
    }
}

fn feature_index(piece: Piece, square: Square) -> usize {
    let piece_type_idx = piece.piece_type() as usize;
    let color_idx = piece.color() as usize;
    (color_idx * PieceType::NUM + piece_type_idx) * 64 + square as usize
}

fn mirrored_index(piece: Piece, square: Square) -> usize {
    let mirrored_piece = Piece::new(!piece.color(), piece.piece_type());
    feature_index(mirrored_piece, square.flip_rank())
}

fn add_at(acc: &mut Accumulator, params: &NNUEParams, piece: Piece, square: Square) {
    forward::add_feature(acc, params, piece, square);
}

fn remove_at(acc: &mut Accumulator, params: &NNUEParams, piece: Piece, square: Square) {
    forward::remove_feature(acc, params, piece, square);
}


/// The feature changes for `mv`, given `board` in its PRE-move state.
/// Applying these as-is performs the "make" update; applying them with
/// polarity flipped performs "undo" — valid because `board.undo_move`
/// restores this exact pre-move state, so calling this again afterward
/// yields the identical list.
fn move_feature_ops(mv: Move, board: &Board) -> [Option<(Piece, Square, bool)>; 4] {
    let mut ops: [Option<(Piece, Square, bool)>; 4] = [None; 4];
    let mut n = 0;
    let mut push = |piece: Piece, square: Square, is_add: bool| {
        ops[n] = Some((piece, square, is_add));
        n += 1;
    };

    let from = mv.from();
    let to = mv.to();
    let stm = board.side_to_move();
    let moving_piece = board.get_piece_on_square(from);

    push(moving_piece, from, false); // always leaves `from`

    if mv.is_castling() {
        let (rook_from, rook_to) = castling_rook_squares(stm, mv.flag());
        let rook = board.get_piece_on_square(rook_from);
        push(moving_piece, to, true);
        push(rook, rook_from, false);
        push(rook, rook_to, true);
    } else if mv.is_en_passant() {
        push(moving_piece, to, true);
        let cap_sq = to.shift(-UP_DIR[stm]);
        let captured = board.get_piece_on_square(cap_sq);
        push(captured, cap_sq, false);
    } else {
        if mv.is_capture() {
            let captured = board.get_piece_on_square(to);
            push(captured, to, false);
        }
        if mv.is_promotion() {
            push(Piece::new(stm, mv.promotion_piece_type()), to, true);
        } else {
            push(moving_piece, to, true);
        }
    }

    ops
}

/// Call with `board` in its PRE-move state, before `board.make_move(mv)`.
pub fn apply_move(acc: &mut Accumulator, params: &NNUEParams, mv: Move, board: &Board) {
    for (piece, square, is_add) in move_feature_ops(mv, board).into_iter().flatten() {
        if is_add { add_at(acc, params, piece, square); } else { remove_at(acc, params, piece, square); }
    }
}

/// Call with `board` back in its PRE-move state, after `board.undo_move(mv)`.
pub fn undo_move(acc: &mut Accumulator, params: &NNUEParams, mv: Move, board: &Board) {
    for (piece, square, is_add) in move_feature_ops(mv, board).into_iter().flatten() {
        if is_add { remove_at(acc, params, piece, square); } else { add_at(acc, params, piece, square); } // flipped
    }
}

pub fn evaluate(acc: &Accumulator, board: &Board, params: &NNUEParams) -> i32 {
    forward::output(acc, board.side_to_move(), params)
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
    use super::{feature_index, Accumulator, NNUEParams, HIDDEN_SIZE, QA, QB, SCALE};
    use crate::types::{color::Color, piece::Piece, square::Square};

    pub fn add_feature(acc: &mut Accumulator, params: &NNUEParams, piece: Piece, square: Square) {
        let idx = feature_index(piece, square);
        let mirrored_piece = Piece::new(!piece.color(), piece.piece_type());
        let midx = feature_index(mirrored_piece, square.flip_rank());

        #[cfg(target_feature = "avx2")]
        unsafe {
            avx2::add(acc, params, idx, midx);
        }
        #[cfg(not(target_feature = "avx2"))]
        scalar::add(acc, params, idx, midx);
    }

    pub fn remove_feature(
        acc: &mut Accumulator,
        params: &NNUEParams,
        piece: Piece,
        square: Square,
    ) {
        let idx = feature_index(piece, square);
        let mirrored_piece = Piece::new(!piece.color(), piece.piece_type());
        let midx = feature_index(mirrored_piece, square.flip_rank());

        #[cfg(target_feature = "avx2")]
        unsafe {
            avx2::remove(acc, params, idx, midx);
        }
        #[cfg(not(target_feature = "avx2"))]
        scalar::remove(acc, params, idx, midx);
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
        use super::{Accumulator, NNUEParams, HIDDEN_SIZE, QA, QB, SCALE};
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

        pub fn remove(acc: &mut Accumulator, params: &NNUEParams, idx: usize, midx: usize) {
            let weights = &params.input_weights[idx];
            for i in 0..HIDDEN_SIZE {
                acc.white[i] -= weights[i];
            }

            let mirrored_weights = &params.input_weights[midx];
            for i in 0..HIDDEN_SIZE {
                acc.black[i] -= mirrored_weights[i];
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
        use super::{Accumulator, NNUEParams, HIDDEN_SIZE, QA, QB, SCALE};
        use crate::types::color::Color;
        use std::arch::x86_64::*;

        #[target_feature(enable = "avx2")]
        unsafe fn add_half(half: &mut [i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) {
            for i in (0..HIDDEN_SIZE).step_by(16) {
                let acc = _mm256_loadu_si256(half.as_ptr().add(i) as *const __m256i);
                let w = _mm256_loadu_si256(weights.as_ptr().add(i) as *const __m256i);
                let sum = _mm256_add_epi16(acc, w);
                _mm256_storeu_si256(half.as_mut_ptr().add(i) as *mut __m256i, sum);
            }
        }

        #[target_feature(enable = "avx2")]
        unsafe fn sub_half(half: &mut [i16; HIDDEN_SIZE], weights: &[i16; HIDDEN_SIZE]) {
            for i in (0..HIDDEN_SIZE).step_by(16) {
                let acc = _mm256_loadu_si256(half.as_ptr().add(i) as *const __m256i);
                let w = _mm256_loadu_si256(weights.as_ptr().add(i) as *const __m256i);
                let diff = _mm256_sub_epi16(acc, w);
                _mm256_storeu_si256(half.as_mut_ptr().add(i) as *mut __m256i, diff);
            }
        }

        pub unsafe fn add(acc: &mut Accumulator, params: &NNUEParams, idx: usize, midx: usize) {
            add_half(&mut acc.white, &params.input_weights[idx]);
            add_half(&mut acc.black, &params.input_weights[midx]);
        }

        pub unsafe fn remove(acc: &mut Accumulator, params: &NNUEParams, idx: usize, midx: usize) {
            sub_half(&mut acc.white, &params.input_weights[idx]);
            sub_half(&mut acc.black, &params.input_weights[midx]);
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
