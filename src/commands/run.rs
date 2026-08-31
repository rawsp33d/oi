use std::path::{Path, PathBuf};

use oi::Reported;
use oi::driver::{build_source, run_source, test_source};

/// Run a source file.
pub fn run(file: &Path) -> Result<(), Reported> {
	run_source(&file.display().to_string(), &read(file)?, root(file))
}

/// Compile a source file to a native executable.
pub fn build(file: &Path, out: Option<&Path>) -> Result<(), Reported> {
	let stem = file.file_stem().map(PathBuf::from).unwrap_or_else(|| "main".into());
	build_source(
		&file.display().to_string(),
		&read(file)?,
		root(file),
		out.unwrap_or(&stem),
	)
}

/// Compile a source file and call its `@test` fns.
pub fn test(file: &Path, pattern: Option<&str>) -> Result<(), Reported> {
	test_source(&file.display().to_string(), &read(file)?, root(file), pattern)
}

/// Read a source file.
fn read(file: &Path) -> Result<String, Reported> {
	std::fs::read_to_string(file).map_err(|e| {
		eprintln!("oi: cannot read {}: {e}", file.display());
		Reported
	})
}

fn root(file: &Path) -> &Path {
	file.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."))
}
