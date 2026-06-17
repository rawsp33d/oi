use crate::helpers::*;

#[test]
fn int_add() {
	check("2 + 3", "5");
}

#[test]
fn int_sub() {
	check("10 - 4", "6");
}

#[test]
fn int_mul() {
	check("3 * 4", "12");
}

#[test]
fn int_div() {
	check("10 / 3", "3");
}

#[test]
fn int_mod() {
	check("10 % 7", "3");
}

// truncated remainder: the sign follows the dividend
#[test]
fn mod_negative_dividend() {
	check("-10 % 7", "-3");
}

#[test]
fn mod_negative_divisor() {
	check("10 % -7", "3");
}

// `%` shares precedence with `*` and `/`, looser than unary `-`
#[test]
fn mod_binds_like_mul() {
	check("1 + 10 % 7", "4");
}

#[test]
fn mod_float_unsupported() {
	assert!(fail("10.0 % 3.0").contains("not yet supported on floats"));
}

#[test]
fn float_add() {
	check("1.5 + 2.0", "3.5");
}

#[test]
fn negation() {
	check("-5", "-5");
}
