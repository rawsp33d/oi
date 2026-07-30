use crate::helpers::*;

#[test]
fn string_concat() {
	check(r#""foo" + "bar""#, "foobar");
}

#[test]
fn string_eq_true() {
	check(r#""foo" == "foo""#, "true");
}

#[test]
fn string_eq_false() {
	check(r#""foo" == "bar""#, "false");
}

#[test]
fn string_ne_true() {
	check(r#""foo" != "bar""#, "true");
}

#[test]
fn string_ne_false() {
	check(r#""foo" != "foo""#, "false");
}

#[test]
fn string_in_found() {
	check(r#""foo" in "foobar""#, "true");
}

#[test]
fn string_in_not_found() {
	check(r#""baz" in "foobar""#, "false");
}

#[test]
fn string_in_exact_match() {
	check(r#""foo" in "foo""#, "true");
}

#[test]
fn string_in_empty_value() {
	// empty string is always a substring
	check(r#""" in "foo""#, "true");
}

#[test]
fn string_in_type_mismatch_error() {
	fail_with(r#"42 in "foo""#, "type mismatch");
}

#[test]
fn escapes() {
	check(r#"print("a\nb\tc")"#, "a\nb\tc");
	check(r#"print("q: \" back: \\")"#, r#"q: " back: \"#);
}

#[test]
fn unknown_escape_fails() {
	fail(r#"print("\z")"#);
}

#[test]
fn raw_strings() {
	check(r#"print(r"no\nescape")"#, r"no\nescape");
	check(r#"print(r"C:\Users\{who}")"#, r"C:\Users\{who}");
	check(r#"r"a\b" + "!""#, r"a\b!");
}
