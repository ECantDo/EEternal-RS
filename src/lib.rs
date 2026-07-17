pub mod board;

pub mod types;

pub mod attacking;

pub mod utils;

pub mod uci;

pub fn initialize() {
    attacking::initialize_lookups();
    uci::run_uci();
    // Wowww... amazing ... there is so much here... ;-;
}
