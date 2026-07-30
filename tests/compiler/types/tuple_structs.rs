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

#[test]
fn construct_positional() {
	check(
		indoc! {"
			struct Money(int)
			Money(500).0
		"},
		"500",
	);
	check(
		indoc! {"
			struct Point(x: float, y: float)
			Point(1.0, 2.0).y
		"},
		"2.0",
	);
}

#[test]
fn construct_named() {
	check(
		indoc! {"
			struct Point(x: float, y: float)
			p := Point(x: 1.0, y: 2.0)
			p.0 == p.x
		"},
		"true",
	);
}

#[test]
fn construct_print() {
	check(
		indoc! {"
			struct Point(x: float, y: float)
			Point(1.0, 2.0)
		"},
		"Point(x: 1.0, y: 2.0)",
	);
}

#[test]
fn nominal_in_signatures() {
	check(
		indoc! {"
			struct Money(int)
			fn pay(m Money) int { m.0 }
			pay(Money(500))
		"},
		"500",
	);
	fail(indoc! {"
		struct Money(int)
		fn pay(m Money) int { m.0 }
		pay(500)
	"});
}

#[test]
fn methods_and_self() {
	check(
		indoc! {"
			struct Money(int)
			impl Money {
				fn double(self) Self {
					Money(self.0 * 2)
				}
			}
			Money(5).double().0
		"},
		"10",
	);
}

#[test]
fn str_override() {
	check(
		indoc! {r#"
			struct Money(int)
			impl Money {
				fn str(self) str {
					"money!"
				}
			}
			print(Money(5))
			Money(5).str()
		"#},
		"money!\nmoney!",
	);
}

#[test]
fn construct_into_sum_member() {
	check(
		indoc! {r#"
			struct UserId(int | string)
			UserId("abc").0
		"#},
		"abc",
	);
}

#[test]
fn wrong_arity_and_type() {
	fail(indoc! {"
		struct Money(int)
		Money(1, 2)
	"});
	fail(indoc! {r#"
		struct Money(int)
		Money("x")
	"#});
}

#[test]
fn builtin_name_errors_at_def() {
	fail_with("struct int(bool)", "is a builtin type");
	fail_with("struct f32(float)", "is a builtin type");
}
