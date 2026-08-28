use std::path::Path;

use oi::Reported;
use oi::driver::{run_source, test_source};

/// Read a source file, then compile and run it.
pub fn run(file: &Path) -> Result<(), Reported> {
	run_source(&file.display().to_string(), &read(file)?, root(file))
}

/// Read a source file, then compile and run its `@test` fns.
pub fn test(file: &Path) -> Result<(), Reported> {
	test_source(&file.display().to_string(), &read(file)?, root(file))
}

fn read(file: &Path) -> Result<String, Reported> {
	std::fs::read_to_string(file).map_err(|e| {
		eprintln!("oi: cannot read {}: {e}", file.display());
		Reported
	})
}

fn root(file: &Path) -> &Path {
	file.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."))
}
