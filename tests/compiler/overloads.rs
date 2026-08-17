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
