use crate::helpers::*;

#[test]
fn zero_value_positional_access() {
	check(
		indoc! {"
			Money :: struct (int)
			m: Money
			m.0
		"},
		"0",
	);
}

#[test]
fn named_and_positional_agree() {
	check(
		indoc! {"
			Point :: struct (x: float, y: float)
			p: Point
			p.x
		"},
		"0.0",
	);
}

#[test]
fn partial_naming() {
	check(
		indoc! {"
			Foo :: struct (int, y: bool)
			f: Foo
			f.y
		"},
		"false",
	);
}

#[test]
fn print_unnamed() {
	check(
		indoc! {"
			Money :: struct (int)
			m: Money
			m
		"},
		"Money(0)",
	);
}

#[test]
fn print_named() {
	check(
		indoc! {"
			Point :: struct (x: float, y: float)
			p: Point
			p
		"},
		"Point(x = 0.0, y = 0.0)",
	);
}

#[test]
fn index_out_of_range() {
	fail(indoc! {"
		Money :: struct (int)
		m: Money
		m.1
	"});
}

#[test]
fn no_such_field() {
	fail(indoc! {"
		Point :: struct (x: float, y: float)
		p: Point
		p.z
	"});
}

#[test]
fn field_type_from_alias() {
	check(
		indoc! {"
			Id :: int
			W :: struct (Id)
			w: W
			w.0
		"},
		"0",
	);
}

#[test]
fn wraps_anonymous_sum() {
	check(
		indoc! {"
			UserId :: struct (int | string)
			u: UserId
			u.0
		"},
		"0",
	);
}

#[test]
fn construct_positional() {
	check(
		indoc! {"
			Money :: struct (int)
			Money(500).0
		"},
		"500",
	);
	check(
		indoc! {"
			Point :: struct (x: float, y: float)
			Point(1.0, 2.0).y
		"},
		"2.0",
	);
}

#[test]
fn construct_named() {
	check(
		indoc! {"
			Point :: struct (x: float, y: float)
			p :: Point(x = 1.0, y = 2.0)
			p.0 == p.x
		"},
		"true",
	);
}

#[test]
fn construct_print() {
	check(
		indoc! {"
			Point :: struct (x: float, y: float)
			Point(1.0, 2.0)
		"},
		"Point(x = 1.0, y = 2.0)",
	);
}

#[test]
fn nominal_in_signatures() {
	check(
		indoc! {"
			Money :: struct (int)
			pay :: fn(m: Money) int { m.0 }
			pay(Money(500))
		"},
		"500",
	);
	fail(indoc! {"
		Money :: struct (int)
		pay :: fn(m: Money) int { m.0 }
		pay(500)
	"});
}

#[test]
fn methods_and_self() {
	check(
		indoc! {"
			Money :: struct (int)
			Money :{
				double :: fn(self) Self {
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
			Money :: struct (int)
			Money :{
				str :: fn(self) str {
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
			UserId :: struct (int | string)
			UserId("abc").0
		"#},
		"abc",
	);
}

#[test]
fn wrong_arity_and_type() {
	fail(indoc! {"
		Money :: struct (int)
		Money(1, 2)
	"});
	fail(indoc! {r#"
		Money :: struct (int)
		Money("x")
	"#});
}

#[test]
fn builtin_name_errors_at_def() {
	fail_with("int :: struct (bool)", "is a builtin type");
	fail_with("f32 :: struct (float)", "is a builtin type");
}
