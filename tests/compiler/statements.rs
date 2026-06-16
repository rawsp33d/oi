use crate::helpers::*;

#[test]
fn stmts() {
	let src = indoc! {"
		x := 3
		y := x * x
		z := y + x
		z
	"};
	check(src, "12");
}

#[test]
fn semicolons_join_lines() {
	// `;` separates statements, so the whole sequence fits on one line
	check("x := 3; y := x * x; y + x", "12");
}

#[test]
fn semicolon_terminator() {
	// a trailing `;` is allowed
	check("1 + 1;", "2");
}
