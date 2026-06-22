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
