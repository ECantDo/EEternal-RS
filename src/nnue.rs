// Architecture ported from Reckless: the accumulator lives on the search thread
// (as a `Network`), not on `Board`. `push`/`pop` just record *what* is about to
// change; the actual feature add/remove math only happens lazily inside
// `evaluate()`, which walks back to the last accurate ply on the stack and
// replays deltas forward. Internal search nodes that never call `evaluate()`
// (i.e. every non-leaf node) pay zero NNUE cost.
//
// Topology is deliberately left as-is: single (768-128)x2-1 SCReLU net, no
// king buckets, no threat inputs, no L2/L3. Because there are no buckets,
// incremental replay is always valid - there's no "can't update, must refresh"
// case the way Reckless needs for king-bucket crossings.

use std::mem::transmute;
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

const ACTUAL: usize = include_bytes!("nnue/768-128x2-1.bin").len();
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
        static EMBEDDED: NNUEParams =
            unsafe { transmute(*include_bytes!("nnue/768-128x2-1.bin")) };
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

#[derive(Clone, Copy, Default)]
struct StackEntry {
    accumulator: Accumulator,
    accurate: bool,
    delta: MoveDelta,
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

        for i in 0..Square::NUM as u8 {
            let square = Square::new(i);
            let piece = board.get_piece_on_square(square);
            if piece != Piece::None {
                forward::add_feature(&mut acc, &params, piece, square);
            }
        }

        self.stack[self.index].accumulator = acc;
        self.stack[self.index].accurate = true;
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

        self.ensure_accurate(&params);

        let acc = &self.stack[self.index].accumulator;
        forward::output(acc, board.side_to_move(), &params)
    }

    fn ensure_accurate(&mut self, params: &Arc<NNUEParams>) {
        if self.stack[self.index].accurate {
            return;
        }

        let mut last_accurate = self.index;
        while !self.stack[last_accurate].accurate {
            debug_assert!(last_accurate > 0, "no accurate accumulator found on stack");
            last_accurate -= 1;
        }

        for i in (last_accurate + 1)..=self.index {
            let mut acc = self.stack[i - 1].accumulator;
            let delta = self.stack[i].delta;
            Self::apply_delta(&mut acc, params, delta);
            self.stack[i].accumulator = acc;
            self.stack[i].accurate = true;
        }
    }

    /// Replays one ply's feature changes, branching on move type exactly the
    /// way `Board::make_move` does - just toggling accumulator features
    /// instead of bitboards.
    fn apply_delta(acc: &mut Accumulator, params: &NNUEParams, delta: MoveDelta) {
        let mv = delta.mv;
        let piece = delta.piece;
        let from = mv.from();
        let to = mv.to();
        let stm = piece.color();

        if mv.is_castling() {
            let (rook_from, rook_to) = castling_rook_squares(stm, mv.flag());
            forward::remove_feature(acc, params, piece, from);
            forward::add_feature(acc, params, piece, to);
            let rook = Piece::new(stm, PieceType::Rook);
            forward::remove_feature(acc, params, rook, rook_from);
            forward::add_feature(acc, params, rook, rook_to);
        } else if mv.is_en_passant() {
            forward::remove_feature(acc, params, piece, from);
            forward::add_feature(acc, params, piece, to);
            let cap_sq = to.shift(-UP_DIR[stm]);
            let captured_pawn = Piece::new(!stm, PieceType::Pawn);
            forward::remove_feature(acc, params, captured_pawn, cap_sq);
        } else {
            if mv.is_capture() {
                forward::remove_feature(acc, params, delta.captured, to);
            }
            forward::remove_feature(acc, params, piece, from);

            if mv.is_promotion() {
                let promoted = Piece::new(stm, mv.promotion_piece_type());
                forward::add_feature(acc, params, promoted, to);
            } else {
                forward::add_feature(acc, params, piece, to);
            }
        }
    }
}

impl Default for NNUE {
    fn default() -> Self {
        Self::new()
    }
}
