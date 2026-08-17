use crate::helpers::*;

#[test]
fn struct_op_dispatches() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		Point : Add {
			add :: fn(self, other: Self) Self {
				Self.{ self.x + other.x, self.y + other.y }
			}
		}
		print(Point.{1, 0} + .{2, 3})
	"};
	check(src, "Point.{x = 3, y = 3}");
}

#[test]
fn eq_dispatches_to_the_fill() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		Point : Add {
			add :: fn(self, other: Self) Self {
				Self.{ self.x + other.x, self.y + other.y }
			}
		}
		Point : Eq {
			eq :: fn(self, other: Self) bool { self.x == other.x && self.y == other.y }
		}
		assert(Point.{1, 0} + Point.{2, 3} == Point.{3, 3})
		print(Point.{1, 0} != .{1, 0})
	"};
	check(src, "false");
}

#[test]
fn eq_is_structural_by_default() {
	let src = indoc! {r#"
		Inner :: struct { n: int }
		Rec :: struct { a: int, s: str, f: float, i: Inner }
		a := Rec.{1, "hi", 1.5, Inner.{2}}
		b := Rec.{1, "hi", 1.5, Inner.{2}}
		c := Rec.{1, "hi", 1.5, Inner.{3}}
		print(a == b)
		print(a != c)
	"#};
	check(src, "true\ntrue");
}

#[test]
fn claimed_eq_beats_the_structural_default() {
	let src = indoc! {"
		Frac :: struct { num: int, den: int }
		Frac : Eq {
			eq :: fn(self, other: Self) bool { self.num * other.den == other.num * self.den }
		}
		print(Frac.{1, 2} == Frac.{2, 4})
	"};
	check(src, "true");
}

#[test]
fn incomparable_field_is_rejected() {
	let src = indoc! {"
		Holder :: struct { f: fn(int) int }
		a := Holder.{ fn(x: int) int { x } }
		a == a
	"};
	fail_with(src, "cannot compare Holder: field `f` is fn(int) int");
}

#[test]
fn unclaimed_struct_is_rejected() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		Point.{1, 2} - Point.{3, 4}
	"};
	fail_with(src, "cannot apply `-` to Point");
}

#[test]
fn fill_must_match_the_trait() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		Point : Add {
			add :: fn(self, other: int) Self { self }
		}
	"};
	fail_with(
		src,
		"`Point.add` is `fn(Point, int) Point`, trait `Add` declares `fn(Point, Point) Point`",
	);
}
