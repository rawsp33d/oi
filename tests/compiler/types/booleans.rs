use crate::helpers::*;

#[test]
fn and_true() {
	check("true && true", "true");
}

#[test]
fn and_false() {
	check("true && false", "false");
}

#[test]
fn or_true() {
	check("false || true", "true");
}

#[test]
fn or_false() {
	check("false || false", "false");
}

#[test]
fn not_true() {
	check("!true", "false");
}

#[test]
fn not_false() {
	check("!false", "true");
}

#[test]
fn chained_and() {
	check("true && true && true", "true");
}

#[test]
fn and_binds_tighter_than_or() {
	check("true || true && false", "true");
}

#[test]
fn not_binds_tighter_than_and() {
	check("!false && false", "false");
}

#[test]
fn comparison_binds_tighter_than_and() {
	check("1 < 2 && 4 > 3", "true");
}

#[test]
fn and_short_circuits() {
	check("false && 1 / 0 > 0", "false");
}

#[test]
fn or_short_circuits() {
	check("true || 1 / 0 > 0", "true");
}

#[test]
fn and_requires_bool() {
	fail_with("1 && true", "expected Bool");
}

#[test]
fn not_requires_bool() {
	fail_with("!1", "expected Bool");
}
