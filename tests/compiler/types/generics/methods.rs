use crate::helpers::*;

#[test]
fn basic_dispatch() {
	let src = indoc! {"
		Box[T] :: struct { v: T }
		impl Box[T] { get :: fn(self) T { self.v } }
		Box{ v: 7 }.get()
	"};
	check(src, "7");
}

#[test]
fn two_instances_coexist() {
	let src = indoc! {r#"
		Box[T] :: struct { v: T }
		impl Box[T] { get :: fn(self) T { self.v } }
		print(Box{ v: 1 }.get())
		print(Box{ v: "hi" }.get())
	"#};
	check(src, "1\nhi");
}

#[test]
fn method_own_type_param() {
	let src = indoc! {r#"
		Box[T] :: struct { v: T }
		impl Box[T] { swap[U] :: fn(self, u: U) U { u } }
		Box{ v: 1 }.swap("hi")
	"#};
	check(src, "hi");
}

#[test]
fn self_return() {
	let src = indoc! {"
		Box[T] :: struct { v: T }
		impl Box[T] { same :: fn(self) Self { self } }
		Box{ v: 3 }.same().v
	"};
	check(src, "3");
}

#[test]
fn concrete_impl_own_type_param() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		impl Point { id[U] :: fn(self, u: U) U { u } }
		Point{1, 2}.id(5)
	"};
	check(src, "5");
}

#[test]
fn unknown_method_error() {
	fail_with(
		indoc! {"
			Box[T] :: struct { v: T }
			impl Box[T] { get :: fn(self) T { self.v } }
			Box{ v: 1 }.nope()
		"},
		"no such method",
	);
}

#[test]
fn field_through_self() {
	let src = indoc! {"
		Box[T] :: struct { v: T }
		impl Box[T] { double :: fn(self) T { self.v + self.v } }
		Box{ v: 7 }.double()
	"};
	check(src, "14");
}
