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
    #[cfg(feature = "embed-nnue")]
    if nnue::try_init_embedded() {
        println!("info string Loaded embedded NNUE network");
    } else {
        println!("info string No NNUE network found, using classical evaluation");
    }
    uci::run_uci();
}
