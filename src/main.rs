fn main() {
	// the compile-time CPU flags decide the NNUE code path (and the default net), not the `avx2` cargo feature
	#[cfg(target_feature = "avx2")]
	println!("Using AVX2");
	#[cfg(not(target_feature = "avx2"))]
	println!("Not using AVX2 (scalar NNUE)");
	#[cfg(feature = "embed-nnue")]
	println!("Attempting to load embedded nnue...");
	#[cfg(not(feature = "embed-nnue"))]
	println!("Not loading NNUE");

	#[cfg(feature = "nightly")]
	println!("Using `nightly`");

	eeternal_rs::initialize();
	eeternal_rs::uci::run_uci();
}
