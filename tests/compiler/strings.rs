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
	assert!(fail(r#"42 in "foo""#).contains("type mismatch"));
}
