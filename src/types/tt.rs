use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use crate::types::moves::Move;

const AGE_CYCLE: u8 = 1 << 5;
const AGE_MASK: u8 = AGE_CYCLE - 1;

const CLUSTER_SIZE: usize = 3;


#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum Bound {
    None = 0, // Use as an `is_empty` check
    Exact = 1,
    Lower = 2, // fail-high: true score >= stored score
    Upper = 3, // fail-low:  true score <= stored score
}

#[derive(Copy, Clone)]
pub struct Flags {
    data: u8, // <is empty?>
}

impl Flags {
    pub const fn new(bound: Bound, tt_pv: bool, age: u8) -> Self {
        debug_assert!(age <= AGE_MASK);

        Self {
            data: (bound as u8) | ((tt_pv as u8) << 2) | (age << 3),
        }
    }

    pub const fn empty() -> Self {
        Self::new(Bound::None, false, 0)
    }

    pub const fn bound(self) -> Bound {
        match self.data & 0b11 {
            0 => Bound::None, // USE FOR checking if exists
            1 => Bound::Exact,
            2 => Bound::Lower,
            3 => Bound::Upper,
            _ => unreachable!(),
        }
    }

    pub const fn tt_pv(self) -> bool {
        (self.data & (1 << 2)) != 0
    }

    pub const fn age(self) -> u8 {
        self.data >> 3
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
struct TTEntry {
    // 8 bytes -> 64 bits
    verify: u16,  // 2
    mv: Move,     // 2
    score: i16,   // 2
    depth: u8,    // 1
    flags: Flags, // 1
}

const _: () = assert!(std::mem::size_of::<TTEntry>() == 8);

impl TTEntry {
    pub fn new(verification: u16, mv: Move, score: i16, depth: u8, bound: Bound, age: u8) -> Self {
        Self {
            verify: verification,
            mv,
            score,
            depth,
            flags: Flags::new(bound, false, age),
        }
    }

    fn empty() -> Self {
        Self {
            verify: 0,
            mv: Move::NONE,
            score: 0,
            depth: 0,
            flags: Flags::empty(),
        }
    }

    // Struct <-> u64 is just a reinterpretation of the same 8 bytes — no
    // manual shifting, the fields are already laid out by `repr(C)`.
    fn to_bits(self) -> u64 {
        unsafe { std::mem::transmute(self) }
    }

    fn from_bits(bits: u64) -> Self {
        unsafe { std::mem::transmute(bits) }
    }
}

#[repr(align(32), C)]
struct TTCluster {
    entries: [AtomicU64; CLUSTER_SIZE],
    // keys: u64,
}


impl TTCluster {
    fn empty() -> Self {
        let empty = TTEntry::empty().to_bits();
        Self {
            entries: [AtomicU64::new(empty), AtomicU64::new(empty), AtomicU64::new(empty)],
        }
    }

    fn load(&self, i: usize) -> TTEntry {
        TTEntry::from_bits(self.entries[i].load(Ordering::Relaxed))
    }

    fn store(&self, i: usize, entry: TTEntry) {
        self.entries[i].store(entry.to_bits(), Ordering::Relaxed);
    }
}

pub struct ProbeResult {
    pub score: Option<i32>,
    pub best_move: Move,
    pub depth: i32, // -1 if nothing found
}

pub struct TranspositionTable {
    clusters: Vec<TTCluster>,
    age: AtomicU8,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        let cluster_bytes = std::mem::size_of::<TTCluster>();
        let num_clusters = (size_mb * 1024 * 1024 / cluster_bytes).max(1);
        let mut clusters = Vec::with_capacity(num_clusters);
        clusters.resize_with(num_clusters, TTCluster::empty);
        Self { clusters, age: AtomicU8::new(0) }
    }

    pub fn resize(&mut self, size_mb: usize) {
        *self = Self::new(size_mb);
    }

    pub fn clear(&mut self) {
        self.clusters.iter_mut().for_each(|c| *c = TTCluster::empty());
        self.age.store(0, Ordering::Relaxed);
    }

    pub fn new_search(&self) {
        let age = self.age.load(Ordering::Relaxed);
        self.age.store((age + 1) & AGE_MASK, Ordering::Relaxed);
    }

    fn index(&self, key: u64) -> usize {
        (((key as u128) * (self.clusters.len() as u128)) >> 64) as usize
    }

    fn verification_key(key: u64) -> u16 {
        key as u16
    }

    fn to_tt_score(score: i32, ply: i32) -> i32 {
        use crate::types::score::Score;
        if score >= Score::MATE_IN_MAX {
            score + ply
        } else if score <= -Score::MATE_IN_MAX {
            score - ply
        } else {
            score
        }
    }

    fn from_tt_score(score: i32, ply: i32) -> i32 {
        use crate::types::score::Score;
        if score >= Score::MATE_IN_MAX {
            score - ply
        } else if score <= -Score::MATE_IN_MAX {
            score + ply
        } else {
            score
        }
    }

    pub fn probe(&self, key: u64, depth: i32, alpha: i32, beta: i32, ply: i32) -> ProbeResult {
        let cluster = &self.clusters[self.index(key)];
        let verify = Self::verification_key(key);

        for i in 0..CLUSTER_SIZE {
            let entry = cluster.load(i);
            if entry.flags.bound() == Bound::None || entry.verify != verify {
                continue;
            }

            let entry_depth = entry.depth as i32;
            if entry_depth < depth {
                return ProbeResult { score: None, best_move: entry.mv, depth: entry_depth };
            }

            let score = Self::from_tt_score(entry.score as i32, ply);
            let usable = match entry.flags.bound() {
                Bound::Exact => true,
                Bound::Upper => score <= alpha,
                Bound::Lower => score >= beta,
                Bound::None => false,
            };

            return ProbeResult {
                score: if usable { Some(score) } else { None },
                best_move: entry.mv,
                depth: entry_depth,
            };
        }

        ProbeResult { score: None, best_move: Move::NONE, depth: -1 }
    }

    pub fn store(&self, key: u64, mv: Move, depth: i32, score: i32, bound: Bound, ply: i32) {
        let cluster = &self.clusters[self.index(key)];
        let verify = Self::verification_key(key);
        let age = self.age.load(Ordering::Relaxed);
        let depth_u8 = depth.clamp(0, u8::MAX as i32) as u8;
        let stored_score = Self::to_tt_score(score, ply).clamp(i16::MIN as i32, i16::MAX as i32) as i16;

        let mut target: Option<usize> = None;
        for i in 0..CLUSTER_SIZE {
            let existing = cluster.load(i);
            if existing.flags.bound() == Bound::None {
                target = Some(i);
                break;
            }
            if existing.verify == verify {
                if existing.depth as i32 > depth && existing.flags.age() == age {
                    return;
                }
                target = Some(i);
                break;
            }
        }

        let target = target.unwrap_or_else(|| {
            (0..CLUSTER_SIZE)
                .min_by_key(|&i| {
                    let e = cluster.load(i);
                    let staleness = age.wrapping_sub(e.flags.age()) as i32 & AGE_MASK as i32;
                    e.depth as i32 - staleness * 4
                })
                .unwrap()
        });

        let existing = cluster.load(target);
        let mv = if mv == Move::NONE && existing.verify == verify { existing.mv } else { mv };

        cluster.store(target, TTEntry::new(verify, mv, stored_score, depth_u8, bound, age));
    }

    pub fn hashfull(&self) -> usize {
        let age = self.age.load(Ordering::Relaxed);
        let mut total = 0;
        let mut filled = 0;
        for cluster in self.clusters.iter().take(1000) {
            for i in 0..CLUSTER_SIZE {
                total += 1;
                let e = cluster.load(i);
                if e.flags.bound() != Bound::None && e.flags.age() == age {
                    filled += 1;
                }
            }
        }
        if total == 0 { 0 } else { filled * 1000 / total }
    }
}