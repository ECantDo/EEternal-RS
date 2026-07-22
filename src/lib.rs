pub mod board;

pub mod types;

pub mod attacking;

pub mod utils;

pub mod uci;

pub mod search;

pub mod nnue;

pub mod time_manager;

pub mod rays;

pub fn initialize() {
    attacking::initialize_lookups();
    #[cfg(feature = "embed-nnue")]
    if nnue::try_init_embedded() {
        println!("Loaded embedded NNUE network");
    } else {
        println!("No NNUE network found, using classical evaluation");
    }
    uci::run_uci();
}
