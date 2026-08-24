use std::path::PathBuf;

use crate::common::{Run, oi, ok};

/// Self-hosted Oi tests.
/// Runs tests in `tests/core/`.
#[test]
fn run_core_tests() {
	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/core");
	let mut paths: Vec<_> = std::fs::read_dir(&dir)
		.unwrap()
		.map(|e| e.unwrap().path())
		.filter(|p| p.extension().is_some_and(|e| e == "oi"))
		.collect();
	paths.sort();
	assert!(!paths.is_empty(), "no .oi files found in tests/core");
	for path in paths {
		ok(oi(&["run", path.to_str().unwrap()]).run(None));
	}
}
