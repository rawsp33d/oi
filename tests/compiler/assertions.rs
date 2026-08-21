use crate::helpers::*;

#[test]
fn assert_true() {
	check("assert!(true)", "");
}
#[test]
fn assert_as_statement() {
	check("assert!(2 > 1)", "");
}
#[test]
fn assert_false_aborts() {
	fail_with("assert!(false)", "assertion failed: false");
}
#[test]
fn assert_false_with_message() {
	fail_with(r#"assert!(false, "bad value")"#, "bad value");
}
#[test]
fn assert_wrong_arg_count() {
	fail_with("assert!()", "1 or 2 arguments");
	fail_with(r#"assert!(true, "a", "b")"#, "1 or 2 arguments");
}
#[test]
fn assert_non_bool_condition() {
	fail_with("assert!(1)", "must be Bool");
}
#[test]
fn assert_non_str_message() {
	fail_with("assert!(false, 42)", "must be Str");
}
#[test]
fn panic_aborts_with_message() {
	fail_with(r#"panic!("uh oh")"#, "panic: uh oh");
}
#[test]
fn panic_no_message() {
	fail_with("panic!()", "panic: panicked");
}
#[test]
fn panic_wrong_arg_count() {
	fail_with(r#"panic!("a", "b")"#, "0 or 1 arguments");
}
#[test]
fn panic_non_str_message() {
	fail_with("panic!(42)", "must be Str");
}
