use crate::common::{Project, Run, oi, ok};

#[test]
fn missing_file_errors() {
	let out = oi(&["run", "definitely-missing.oi"]).run(None);
	assert!(!out.status.success());
	let stderr = String::from_utf8_lossy(&out.stderr);
	assert!(stderr.contains("cannot read"), "stderr was:\n{stderr}");
}

#[test]
fn default_file_is_main_oi_in_cwd() {
	// with no path, `run` runs ./main.oi in the current directory
	let dir = Project::new(&[("main.oi", "1 + 2")]);
	assert_eq!(ok(oi(&["run"]).current_dir(&dir).run(None)), "3");
}

#[test]
fn debug_ast_goes_to_stderr() {
	// --debug-ast dumps the AST to stderr
	let plain = oi(&["run", "examples/main.oi"]).run(None);
	assert!(plain.status.success());
	assert!(
		plain.stderr.is_empty(),
		"unexpected stderr:\n{}",
		String::from_utf8_lossy(&plain.stderr)
	);

	let dumped = oi(&["run", "examples/main.oi", "--debug-ast"]).run(None);
	assert!(dumped.status.success());
	assert!(!dumped.stderr.is_empty(), "expected the AST dump on stderr");
	assert_eq!(dumped.stdout, plain.stdout);
}
