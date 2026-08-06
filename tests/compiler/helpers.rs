use std::process::Output;

pub(crate) use indoc::indoc;
use pretty_assertions::assert_eq;

use crate::common::{oi, stdout_ok, trim_trailing_newline};

/// Run `src` through `oi exec`.
fn exec(src: &str) -> Output {
	oi(&["exec"], Some(src))
}

/// Run provided source, returning trimmed stdout.
pub(crate) fn run(src: &str) -> String {
	stdout_ok(exec(src))
}

/// Run provided source, returning (trimmed stdout, raw stderr).
pub(crate) fn run_streams(src: &str) -> (String, String) {
	let out = exec(src);
	(trim_trailing_newline(&out.stdout), trim_trailing_newline(&out.stderr))
}

/// Run provided source expecting a compilation error.
pub(crate) fn fail(src: impl Lines) -> String {
	let src = src.text();
	let out = exec(&src);
	assert!(
		!out.status.success(),
		"expected failure but compiler succeeded\nsrc:\n{src}\nstdout:\n{}",
		String::from_utf8_lossy(&out.stdout)
	);
	trim_trailing_newline(&out.stderr)
}

/// Text joined by newlines.
pub(crate) trait Lines {
	fn text(&self) -> String;
}

impl Lines for &str {
	fn text(&self) -> String {
		(*self).to_string()
	}
}

impl Lines for &String {
	fn text(&self) -> String {
		(*self).clone()
	}
}

impl<const N: usize> Lines for [&str; N] {
	fn text(&self) -> String {
		self.join("\n")
	}
}

/// Run provided source expecting a compilation error containing `expected`.
pub(crate) fn fail_with(src: impl Lines, expected: &str) {
	let src = src.text();
	let err = fail(&src);
	assert!(
		err.contains(expected),
		"\nexpected error containing {expected:?}\nsrc:\n{src}\nstderr:\n{err}"
	);
}

/// Run provided source expecting a given result.
pub(crate) fn check(src: impl Lines, expected: impl Lines) {
	let src = src.text();
	assert_eq!(run(&src), expected.text(), "\nsrc:\n{src}");
}

/// Run under the leak checker, returning the live-allocation count at exit.
pub(crate) fn leaks(src: impl Lines) -> i64 {
	let src = src.text();
	let out = crate::common::oi_env(&[("OI_LEAK_CHECK", "1")], &["exec"], Some(&src));
	assert!(
		out.status.success(),
		"src:\n{src}\nstderr:\n{}",
		String::from_utf8_lossy(&out.stderr)
	);
	let err = String::from_utf8_lossy(&out.stderr).to_string();
	let count = err
		.lines()
		.find_map(|l| l.strip_prefix("leaked allocations: "))
		.unwrap_or_else(|| panic!("no leak report\nsrc:\n{src}\nstderr:\n{err}"));
	count.parse().unwrap()
}

/// Run and assert every allocation was freed.
pub(crate) fn assert_clean(src: impl Lines) {
	let src = src.text();
	assert_eq!(leaks(&src), 0, "leaked\nsrc:\n{src}");
}
