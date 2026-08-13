use crate::helpers::*;

#[test]
fn instance_method() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		Point :{
			sum :: fn(self) int { self.x + self.y }
		}
		p :: Point.{3, 4}
		p.sum()
	"};
	check(src, "7");
}

#[test]
fn method_with_args() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		Point :{
			scaled :: fn(self, k: int) int { (self.x + self.y) * k }
		}
		Point.{3, 4}.scaled(10)
	"};
	check(src, "70");
}

#[test]
fn method_on_literal() {
	let src = indoc! {"
		P :: struct { x: int, y: int }
		P :{ sum :: fn(self) int { self.x + self.y } }
		P.{3, 4}.sum()
	"};
	check(src, "7");
}

#[test]
fn method_returns_struct_field() {
	let src = indoc! {r#"
		User :: struct { name: string, age: int }
		User :{
			can_register :: fn(self) bool { self.age > 16 }
		}
		User.{name = "ada", age = 36}.can_register()
	"#};
	check(src, "true");
}

#[test]
fn static_method() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		Point :{
			origin :: fn() Point { Point.{0, 0} }
			sum :: fn(self) int { self.x + self.y }
		}
		Point.origin().sum()
	"};
	check(src, "0");
}

#[test]
fn static_method_with_args() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		Point :{ make :: fn(a: int, b: int) Point { Point.{a, b} } }
		Point.make(3, 4).x
	"};
	check(src, "3");
}

#[test]
fn self_type_and_literal() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		Point :{
			new :: fn() Self { Self.{} }
			sum :: fn(self) int { self.x + self.y }
		}
		Point.new().sum()
	"};
	check(src, "0");
}

#[test]
fn self_param_and_fields() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		Point :{
			add :: fn(self, other: Self) Self { Self.{self.x + other.x, self.y + other.y} }
		}
		Point.{1, 2}.add(Point.{3, 4}).x
	"};
	check(src, "4");
}

#[test]
fn self_outside_impl() {
	fail_with("Self.{}", "no enclosing impl");
}

#[test]
fn mut_self_mutates_receiver() {
	let src = indoc! {"
		Counter :: struct { n: int }
		Counter :{
			bump :: fn(mut self) { self.n = self.n + 1 }
			get :: fn(self) int { self.n }
		}
		c := Counter.{0}
		c.bump()
		c.bump()
		c.get()
	"};
	check(src, "2");
}

#[test]
fn immutable_self_rejects_field_assign() {
	fail_with(
		indoc! {"
			P :: struct { x: int }
			P :{ bad :: fn(self) { self.x = 9 } }
			P.{1}.bad()
		"},
		"immutably bound",
	);
}

#[test]
fn no_such_method() {
	fail_with(
		indoc! {"
			P :: struct { x: int }
			p :: P.{1}
			p.nope()
		"},
		"no method `nope`",
	);
}

#[test]
fn methods_only_on_structs() {
	fail_with("(5).double()", "no methods");
}

#[test]
fn wrong_arg_count() {
	fail_with(
		indoc! {"
			P :: struct { x: int }
			P :{ add :: fn(self, k: int) int { self.x + k } }
			P.{1}.add()
		"},
		"expects 1 argument",
	);
}
