use crate::helpers::*;

#[test]
fn variable() {
	check("x :: 42\nx", "42");
}

#[test]
fn assign() {
	check(["x := 1", "x = 2", "x"], "2");
}

#[test]
fn assign_from_self() {
	check(["x := 10", "x = x + 5", "x"], "15");
}

#[test]
fn assign_string() {
	check(r#"s := "old"; s = "new"; s"#, "new");
}

#[test]
fn declare_zero_int() {
	check("n: int\nn", "0");
}

#[test]
fn declare_zero_string() {
	check("s: string\ns", "");
}

#[test]
fn declare_zero_then_assign() {
	check(["n: int", "n = 7", "n"], "7");
}

#[test]
fn declare_zero_struct() {
	check(
		indoc! {"
			Point :: struct { x: int, y: int }
			p: Point
			p.x = 5
			p.x
		"},
		"5",
	);
}

#[test]
fn annotated_binding() {
	check("a : int : 2\na", "2");
	check(r#"b : string : "hi"; b"#, "hi");
}

#[test]
fn annotation_type_mismatch() {
	fail_with(r#"x : int : "hi""#, "expected int, got string");
}

#[test]
fn annotation_pins_width() {
	// the literal fits an i32, but the annotation widens it to i64
	check("big : i64 : 50_000\nbig", "50000");
}

#[test]
fn annotation_coerces_float() {
	check("f : f32 : 1.5\nf", "1.5");
	check("x : f64 : 5\nx", "5.0");
}

#[test]
fn annotation_out_of_range() {
	fail_with("x : i8 : 9999\nx", "out of range for i8");
}
