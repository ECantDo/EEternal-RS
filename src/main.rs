fn main() {
    #[cfg(feature = "avx2")]
    println!("Using AVX2");
    #[cfg(not(feature = "avx2"))]
    println!("Not using AVX2");
    #[cfg(feature = "embed-nnue")]
    println!("Attempting to load embedded nnue...");
    #[cfg(not(feature = "embed-nnue"))]
    println!("Not loading NNUE");

    eeternal_rs::initialize();
    eeternal_rs::uci::run_uci();
}
