use crate::helpers::*;
use indoc::indoc;

#[test]
fn field_access() {
	check(
		"Point :: struct { x: int, y: int }
		point :: Point{ x = 1, y = 2 }
		point.x",
		"1",
	);
	check(
		"Point :: struct { x: int, y: int }
		point :: Point{ x = 1, y = 2 }
		point.y",
		"2",
	);
}

#[test]
fn zero_value() {
	check(
		"Point :: struct { x: int, y: int }
		origin :: Point{}
		origin.x",
		"0",
	);
	check(
		"User :: struct { name: string, age: int }
		u :: User{}
		u.age",
		"0",
	);
}

#[test]
fn positional_literal() {
	check(
		"Point :: struct { x: int, y: int }
		p :: Point{3, 4}
		p.x",
		"3",
	);
	check(
		"Point :: struct { x: int, y: int }
		p :: Point{3, 4}
		p.y",
		"4",
	);
}

#[test]
fn field_mutation() {
	check(
		"Point :: struct { x: int, y: int }
		p := Point{}
		p.x = 5
		p.x",
		"5",
	);
	check(
		"Point :: struct { x: int, y: int }
		p := Point{ x = 10, y = 20 }
		p.y = 99
		p.y",
		"99",
	);
}

#[test]
fn copy_semantics() {
	check(
		"Point :: struct { x: int, y: int }
		a :: Point{ x = 1, y = 2 }
		b := a
		b.x = 99
		a.x",
		"1",
	);
}

#[test]
fn copy_of_array_field_is_independent() {
	check(
		"Bag :: struct { items: []int }
		s :: Bag{ items = [1, 2, 3] }
		b := s.items
		b << 4
		s.items",
		"[1, 2, 3]",
	);
}

#[test]
fn struct_lit_copies_array_field() {
	check(
		"Bag :: struct { items: []int }
		a := [1]
		bags :: [Bag{ items = a }]
		a << 2
		bags[0].items",
		"[1]",
	);
}

#[test]
fn print_struct() {
	check(
		"Point :: struct { x: int, y: int }
		print(Point{ x = 1, y = 2 })",
		"Point{x = 1, y = 2}",
	);
	check(
		"Point :: struct { x: int, y: int }
		print(Point{})",
		"Point{x = 0, y = 0}",
	);
}

#[test]
fn mixed_field_types() {
	check(
		r#"Foo :: struct { n: int, s: string, f: float }
		v :: Foo{ n = 42, s = "hi", f = 1.5 }
		v.n"#,
		"42",
	);
	check(
		r#"Foo :: struct { n: int, s: string }
		v :: Foo{ n = 7, s = "world" }
		v.s"#,
		"world",
	);
}

#[test]
fn fn_return_type_annotation() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		origin :: fn() Point { Point{} }
		origin()
	"};
	check(src, "Point{x = 0, y = 0}");
}

#[test]
fn fn_return_type_annotation_named_fields() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		make :: fn(a: int, b: int) Point { Point{ x = a, y = b } }
		make(3, 4)
	"};
	check(src, "Point{x = 3, y = 4}");
}

#[test]
fn fn_return_type_annotation_mismatch() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		bad :: fn() Point { 42 }
		bad()
	"};
	fail_with(src, "wrong return type");
}

#[test]
fn fn_param_struct_type() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		sum :: fn(p: Point) int { p.x + p.y }
		sum(Point{ x = 3, y = 4 })
	"};
	check(src, "7");
}

#[test]
fn if_no_else_struct_zero() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		p :: if false { Point{ x = 1, y = 2 } }
		p.x
	"};
	check(src, "0");
}

#[test]
fn immutable_field_assign_error() {
	fail_with(
		"Point :: struct { x: int, y: int }
		p :: Point{}
		p.x = 5",
		"immutable",
	);
}

#[test]
fn struct_positional_field_access() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		p :: Point{ 2 4 }
		p.1 == p.y
	"};
	check(src, "true");
}

#[test]
fn record_coerces_to_struct() {
	check(
		"Point :: struct { x: int, y: int }
		p : Point : { x = 2, y = 1 }
		p.x + p.y",
		"3",
	);
	check(
		"Point :: struct { x: int, y: int }
		x :: 5
		y :: 7
		p : Point : { x, y }
		p.y",
		"7",
	);
}

#[test]
fn record_as_call_arg() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		sum :: fn(p: Point) int { p.x + p.y }
		sum({ x = 3, y = 4 })
	"};
	check(src, "7");
}

#[test]
fn record_in_return_position() {
	let src = indoc! {"
		Point :: struct { x: int, y: int }
		make :: fn() Point { { x = 1, y = 2 } }
		make()
	"};
	check(src, "Point{x = 1, y = 2}");
}

#[test]
fn empty_record_defaults_struct() {
	check(
		"User :: struct { age: int, swag: int = 5 }
		u : User : {}
		u.swag",
		"5",
	);
}

#[test]
fn record_unknown_field_error() {
	fail_with(
		"Point :: struct { x: int, y: int }
		p : Point : { z = 1 }",
		"no field `z`",
	);
}

#[test]
fn record_non_ident_key_error() {
	fail_with(
		r#"Point :: struct { x: int, y: int }
		p : Point : { "x" = 1 }"#,
		"named by idents",
	);
}

#[test]
fn default_field_value() {
	// empty literal uses the default
	check(
		"User :: struct { age: int, name: string, swag: int = 5 }
		u :: User{}
		u.swag",
		"5",
	);
	// partial named literal
	check(
		"User :: struct { age: int, swag: int = 5 }
		u :: User{ age = 30 }
		u.swag",
		"5",
	);
	// explicit value overrides the default
	check(
		"User :: struct { age: int, swag: int = 5 }
		u :: User{ swag = 99 }
		u.swag",
		"99",
	);
	// non-defaulted fields still zero-init
	check(
		"User :: struct { age: int, swag: int = 5 }
		u :: User{}
		u.age",
		"0",
	);
}

#[test]
fn named_call_args() {
	check(
		"Options :: struct { foo: int, bar: bool }
		f :: fn(o: Options) { print(o.foo) }
		f(bar: true, foo: 4)",
		"4",
	);
}

#[test]
fn named_method_args() {
	check(
		"Options :: struct { foo: int, bar: bool }
		User :: struct {}
		User :{
			with_options :: fn(self, opt: Options) { print(opt.bar) }
		}
		user :: User{}
		user.with_options(bar: true, foo: 4)",
		"true",
	);
}

#[test]
fn mixed_positional_and_named_args() {
	check(
		"Options :: struct { foo: int }
		g :: fn(x: int, o: Options) { print(x + o.foo) }
		g(1, foo: 2)",
		"3",
	);
}

#[test]
fn named_before_positional_error() {
	fail_with(
		"Options :: struct { foo: int }
		g :: fn(x: int, o: Options) {}
		g(foo: 1, 2)",
		"positional args go before named args",
	);
}

#[test]
fn struct_typed_field() {
	let src = indoc! {"
		Money :: struct { amount: int }
		Wallet :: struct { cash: Money }
		w :: Wallet{ cash = Money{ amount = 5 } }
		print(w.cash.amount)
		print(w)
	"};
	check(src, ["5", "Wallet{cash = Money{amount = 5}}"]);
}

#[test]
fn struct_typed_field_out_of_order() {
	let src = indoc! {"
		Wallet :: struct { cash: Money }
		Money :: struct { amount: int }
		Wallet{ cash = Money{ amount = 7 } }.cash.amount
	"};
	check(src, "7");
}

#[test]
fn struct_typed_field_reassign() {
	let src = indoc! {"
		Money :: struct { amount: int }
		Wallet :: struct { cash: Money }
		w := Wallet{ cash = Money{ amount = 5 } }
		w.cash = Money{ amount = 9 }
		w.cash.amount
	"};
	check(src, "9");
}

#[test]
fn self_recursive_struct_error() {
	fail_with("A :: struct { a: A }", "recurses for ever ever");
}

#[test]
fn mutually_recursive_structs_error() {
	fail_with(
		"A :: struct { b: B }
		B :: struct { a: A }",
		"recurses for ever ever",
	);
}

#[test]
fn unknown_field_type_error() {
	fail_with("Wallet :: struct { cash: Money }", "unknown type `Money`");
}
