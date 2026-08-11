use crate::helpers::*;
use indoc::indoc;

#[test]
fn infer_from_literal() {
	let src = indoc! {"
		struct Pair[T] { a: T, b: T }
		p :: Pair{ a: 3, b: 4 }
		p.a + p.b
	"};
	check(src, "7");
}

#[test]
fn nested_instantiation() {
	let src = indoc! {"
		struct Box[T] { v: T }
		Box{ v: Box{ v: 5 } }.v.v
	"};
	check(src, "5");
}

#[test]
fn type_position_param() {
	let src = indoc! {"
		struct Pair[T] { a: T, b: T }
		fn sum(p: Pair[int]) int { p.a + p.b }
		sum(Pair{ a: 3, b: 4 })
	"};
	check(src, "7");
}

#[test]
fn conflicting_field_types_error() {
	fail_with(
		indoc! {r#"
			struct Pair[T] { a: T, b: T }
			Pair{ a: 3, b: "x" }
		"#},
		"bound to both",
	);
}

#[test]
fn cannot_infer_error() {
	fail_with(
		indoc! {"
			struct Pair[T] { a: T, b: T }
			Pair{}
		"},
		"cannot infer",
	);
}

#[test]
fn empty_lit_infers_from_annotation() {
	check(
		indoc! {"
			struct Box[T] { v: T }
			b : Box[int] : Box{}
			b.v
		"},
		"0",
	);
}

#[test]
fn partial_lit_infers_from_annotation() {
	check(
		indoc! {r#"
			struct Pair[A, B] { a: A, b: B }
			p : Pair[int, string] : Pair{ a: 7 }
			p.a
		"#},
		"7",
	);
}

#[test]
fn bare_name_needs_type_arguments() {
	fail_with(
		indoc! {"
			struct Pair[T] { a: T, b: T }
			fn f(p: Pair) int { p.a }
			0
		"},
		"needs type arguments",
	);
}

#[test]
fn generic_fn_round_trip() {
	let src = indoc! {"
		struct Box[T] { v: T }
		fn wrap[T](v: T) Box[T] { Box{ v: v } }
		wrap(9).v
	"};
	check(src, "9");
}

#[test]
fn concrete_field_type_still_checked() {
	fail_with(
		indoc! {r#"
			struct Tagged[T] { v: T, id: int }
			Tagged{ v: 1.5, id: "x" }
		"#},
		"expected int",
	);
}

#[test]
fn type_args_on_non_generic_struct_error() {
	fail_with(
		indoc! {"
			struct Point { x: int, y: int }
			fn f(p: Point[int]) int { p.x }
			0
		"},
		"is not generic",
	);
}
