use std::path::Path;
use std::{env, fs};

fn main() {
	// Re-run this script only when the params file changes.
	println!("cargo:rerun-if-changed=build/params.csv");
	let text = fs::read_to_string("build/params.csv").expect("`build/params.csv` is missing");

	let mut out = String::new();

	for line in text.lines().skip(1) {
		let line = line.trim();
		if line.is_empty() {
			continue;
		}

		let cols: Vec<&str> = line.split(',').map(|c| c.trim()).collect();

		if cols.len() < 3 {
			panic!("bad row: {line}");
		}

		let (name, value, ty) = (cols[0], cols[1], cols[2]);

		out.push_str(&format!("pub const {name}: {ty} = {value};\n"));
	}

	let dest = Path::new(&env::var("OUT_DIR").unwrap()).join("params.rs");
	fs::write(dest, out).unwrap();
}
