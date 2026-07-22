use std::ops::Deref;

pub mod board;

pub mod types;

pub mod attacking;

pub mod utils;

pub mod uci;

pub mod search;

pub mod nnue;

pub mod time_manager;

pub fn initialize() {
    attacking::initialize_lookups();
    #[cfg(not(feature = "embed-nnue"))]
    println!("info not loading NNUE");
    uci::run_uci();
}
