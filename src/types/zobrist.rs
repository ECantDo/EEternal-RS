// Thanks Reckless ... just going to yoink this bit ...
// I mainly want the seed, and it's done in a really clean way.
pub struct Zobrist {
    pub pieces: [[u64; 64]; 12],
    pub en_passant: [u64; 64],
    pub castling: [u64; 16],
    pub side: u64,
    pub fifty_move_clock: [u64; 16],
}

pub const ZOBRIST: Zobrist = {
    const SEED: u64 = 0xFFAA_B58C_5833_FE89u64;
    const INCREMENT: u64 = 0x9E37_79B9_7F4A_7C15;

    // 64×12+64+16+16+1 = 865 (list of u64 of length matching the struct above)
    let mut zobrist = [0; 865];
    let mut state = SEED;

    let mut i = 0;
    while i < zobrist.len() {
        state = state.wrapping_add(INCREMENT);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        zobrist[i] = z ^ (z >> 31);

        i += 1;
    }
    unsafe { std::mem::transmute(zobrist) }
};