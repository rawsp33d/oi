use crate::helpers::*;

#[test]
fn zero_value_positional_access() {
	check(
		indoc! {"
			struct Money(int)
			mut m Money
			m.0
		"},
		"0",
	);
}

#[test]
fn named_and_positional_agree() {
	check(
		indoc! {"
			struct Point(x: float, y: float)
			mut p Point
			p.x
		"},
		"0.0",
	);
}

#[test]
fn partial_naming() {
	check(
		indoc! {"
			struct Foo(int, y: bool)
			mut f Foo
			f.y
		"},
		"false",
	);
}

#[test]
fn print_unnamed() {
	check(
		indoc! {"
			struct Money(int)
			mut m Money
			m
		"},
		"Money(0)",
	);
}

#[test]
fn print_named() {
	check(
		indoc! {"
			struct Point(x: float, y: float)
			mut p Point
			p
		"},
		"Point(x: 0.0, y: 0.0)",
	);
}

#[test]
fn index_out_of_range() {
	fail(indoc! {"
		struct Money(int)
		mut m Money
		m.1
	"});
}

#[test]
fn no_such_field() {
	fail(indoc! {"
		struct Point(x: float, y: float)
		mut p Point
		p.z
	"});
}

#[test]
fn field_type_from_alias() {
	check(
		indoc! {"
			type Id = int
			struct W(Id)
			mut w W
			w.0
		"},
		"0",
	);
}

#[test]
fn wraps_anonymous_sum() {
	check(
		indoc! {"
			struct UserId(int | string)
			mut u UserId
			u.0
		"},
		"0",
	);
}
