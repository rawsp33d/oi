use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use indoc::indoc;

static ID: AtomicUsize = AtomicUsize::new(0);

// Run an Oi script.
fn run(src: &str) -> String {
	let n = ID.fetch_add(1, Ordering::Relaxed);
	let path = std::env::temp_dir().join(format!("oi_test_{n}.oi"));
	std::fs::write(&path, src).unwrap();
	let out = exec(&path);
	std::fs::remove_file(&path).ok();
	out
}

// Run an Oi file.
#[allow(dead_code)]
fn run_file(name: &str) -> String {
	let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("tests/cases")
		.join(name);
	exec(&path)
}

fn exec(path: &Path) -> String {
	let out = Command::new(env!("CARGO_BIN_EXE_oi"))
		.arg(path)
		.output()
		.unwrap();
	assert!(
		out.status.success(),
		"compiler failed:\n{}",
		String::from_utf8_lossy(&out.stderr)
	);
	let s = String::from_utf8(out.stdout).unwrap();
	s.strip_suffix('\n').unwrap_or(&s).to_string()
}

// tests

#[test]
fn int_literal() {
	assert_eq!(run("42"), "42");
}

#[test]
fn float_literal() {
	assert_eq!(run("3.14"), "3.14");
}

#[test]
fn bool_literal() {
	assert_eq!(run("true"), "true");
	assert_eq!(run("false"), "false");
}

#[test]
fn string_literal() {
	assert_eq!(run(r#""hello""#), "hello");
}

#[test]
fn int_arithmetic() {
	assert_eq!(run("2 + 3"), "5");
	assert_eq!(run("10 - 4"), "6");
	assert_eq!(run("3 * 4"), "12");
	assert_eq!(run("10 / 3"), "3");
}

#[test]
fn float_arithmetic() {
	assert_eq!(run("1.5 + 2.0"), "3.5");
}

#[test]
fn negation() {
	assert_eq!(run("-5"), "-5");
}

#[test]
fn string_concat() {
	assert_eq!(run(r#""foo" + "bar""#), "foobar");
}

#[test]
fn variable() {
	assert_eq!(run("x := 42\nx"), "42");
}

#[test]
fn fn_call() {
	assert_eq!(
		run(indoc! {"
			fn double() { 21 * 2 }
			double()
		"}),
		"42"
	);
}

#[test]
fn multi_fn() {
	assert_eq!(
		run(indoc! {"
			fn base() {
				6
			}

			fn triple() {
				base() + base() + base()
			}

			triple()
		"}),
		"18"
	);
}

#[test]
fn fn_vars() {
	assert_eq!(
		run(indoc! {"
			fn area() {
				width := 12
				height := 5
				width * height
			}

			area()
		"}),
		"60"
	);
}

#[test]
fn stmts() {
	assert_eq!(
		run(indoc! {"
			x := 3
			y := x * x
			z := y + x
			z
		"}),
		"12"
	);
}
