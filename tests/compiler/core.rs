use std::path::PathBuf;

use crate::common::{Run, oi, trim};

/// Self-hosted Oi tests.
/// Runs `oi test` over files in `tests/core/`.
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
		let name = path.file_name().unwrap().to_string_lossy().into_owned();
		let out = oi(&["test", path.to_str().unwrap()]).run(None);
		let report = trim(&out.stdout);
		assert!(
			out.status.success(),
			"{name}:\n{report}\n{}",
			String::from_utf8_lossy(&out.stderr)
		);
		assert!(!report.starts_with("0 passed"), "{name}: no @test fns ran");
		println!("{name}\n{report}");
	}
}
