use crate::support::oi;

#[test]
fn missing_file_errors() {
	let out = oi(&["run", "definitely-missing.oi"], None);
	assert!(!out.status.success());
	let stderr = String::from_utf8_lossy(&out.stderr);
	assert!(stderr.contains("cannot read"), "stderr was:\n{stderr}");
}

#[test]
fn default_file_runs() {
	// with no path, `run` falls back to examples/main.oi
	let out = oi(&["run"], None);
	assert!(
		out.status.success(),
		"stderr:\n{}",
		String::from_utf8_lossy(&out.stderr)
	);
	assert!(!String::from_utf8(out.stdout).unwrap().trim().is_empty());
}

#[test]
fn debug_ast_goes_to_stderr() {
	// --debug-ast dumps the AST to stderr
	let plain = oi(&["run", "examples/main.oi"], None);
	assert!(plain.status.success());
	assert!(
		plain.stderr.is_empty(),
		"unexpected stderr:\n{}",
		String::from_utf8_lossy(&plain.stderr)
	);

	let dumped = oi(&["run", "examples/main.oi", "--debug-ast"], None);
	assert!(dumped.status.success());
	assert!(!dumped.stderr.is_empty(), "expected the AST dump on stderr");
	assert_eq!(dumped.stdout, plain.stdout);
}
