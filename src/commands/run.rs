use std::path::{Path, PathBuf};

use oi::Reported;
use oi::driver::{DebugOpts, build_source, run_source, test_source};

/// Resolve the entry file.
pub fn entry(file: Option<PathBuf>) -> PathBuf {
	if let Some(file) = file {
		return file;
	}
	let cwd_path = PathBuf::from("main.oi");
	if cwd_path.exists() {
		cwd_path
	} else {
		PathBuf::from("src/main.oi")
	}
}

/// Run a source file.
pub fn run(file: &Path, timings: bool) -> Result<(), Reported> {
	run_source(
		&file.display().to_string(),
		&read(file)?,
		root(file),
		DebugOpts { timings },
	)
}

/// Compile a source file to a native executable or shared library.
pub fn build(file: &Path, out: Option<&Path>, lib: bool) -> Result<(), Reported> {
	let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("main");
	let default = match lib {
		true => format!("{}{stem}{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX).into(),
		false => PathBuf::from(stem),
	};
	build_source(
		&file.display().to_string(),
		&read(file)?,
		root(file),
		out.unwrap_or(&default),
		lib,
	)
}

/// Compile a source file and call its `@test` fns.
pub fn test(file: &Path, pattern: Option<&str>) -> Result<(), Reported> {
	test_source(&file.display().to_string(), &read(file)?, root(file), pattern)
}

/// Read a source file.
pub fn read(file: &Path) -> Result<String, Reported> {
	std::fs::read_to_string(file).map_err(|e| {
		eprintln!("oi: cannot read {}: {e}", file.display());
		Reported
	})
}

pub fn root(file: &Path) -> &Path {
	file.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."))
}
