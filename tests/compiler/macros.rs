use crate::helpers::*;

#[test]
fn dbg_passes_value_through() {
	check("dbg!(1 + 2)", "3");
}

#[test]
fn dbg_prints_snippet_and_value_to_stderr() {
	let (_, err) = run_streams("dbg!(1 + 2)");
	assert!(err.contains("1 + 2 = 3"), "stderr:\n{err}");
}

#[test]
fn assert_statement_form() {
	check("assert! 1 + 1 == 2", "");
}

#[test]
fn assert_statement_form_fails_with_snippet() {
	fail_with("assert! 1 == 2", "assertion failed: 1 == 2");
}

#[test]
fn helpers_abort() {
	fail_with("todo!()", "not yet implemented");
	fail_with("unreachable!()", "entered unreachable code");
}

#[test]
fn helpers_with_message() {
	fail_with(r#"todo!("idk")"#, "idk");
	fail_with(r#"unreachable!("nope")"#, "nope");
}

#[test]
fn unknown_macro_errors() {
	fail_with("nope!(1)", "no macro named");
}

#[test]
fn bare_assert_call_suggests_macro() {
	fail_with("assert(true)", "write `assert!(...)`");
}

#[test]
fn bare_panic_call_suggests_macro() {
	fail_with(r#"panic("oops")"#, "write `panic!(...)`");
}
