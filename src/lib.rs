#![cfg_attr(feature = "nightly", feature(hint_prefetch))]
pub mod attacking;
pub mod board;
pub mod history;
pub mod nnue;
pub mod params;
pub mod rays;
pub mod search;
pub mod settings;
pub mod time_manager;
pub mod types;
pub mod uci;
pub mod utils;

pub fn initialize() {
	attacking::initialize_lookups();
	#[cfg(feature = "embed-nnue")]
	if nnue::try_init_embedded() {
		println!("Loaded embedded NNUE network");
	} else {
		println!("No NNUE network found, using classical evaluation");
	}

	// Init search stuff
	search::search_types::LMR_TABLE[0][0];
}
