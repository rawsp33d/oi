use crate::helpers::*;

#[test]
fn call_via_var() {
	let src = indoc! {"
		mul :: fn [] (x: int, y: int) int { x * y }
		mul(6, 7)
	"};
	check(src, "42");
}

#[test]
fn call_via_var_passed_to_fn() {
	let src = indoc! {"
		apply :: fn(f: fn(int) int, x: int) int { f(x) }
		double :: fn [] (n: int) int { n * 2 }
		apply(double, 21)
	"};
	check(src, "42");
}

#[test]
fn wrong_arg_count() {
	let src = indoc! {"
		add :: fn [] (x: int, y: int) int { x + y }
		add(1)
	"};
	fail_with(src, "expects 2 argument");
}

#[test]
fn wrong_arg_type() {
	let src = indoc! {"
		add :: fn [] (x: int, y: int) int { x + y }
		add(1, 2.0)
	"};
	fail_with(src, "wrong argument type");
}

#[test]
fn not_callable() {
	let src = indoc! {"
		x :: 5
		x()
	"};
	fail_with(src, "not callable");
}

#[test]
fn capture_read_only() {
	let src = indoc! {"
		factor :: 3
		triple :: fn [factor] (x: int) int { x * factor }
		triple(4)
	"};
	check(src, "12");
}

#[test]
fn capture_move() {
	let src = indoc! {"
		factor :: 3
		triple :: fn [move factor] (x: int) int { x * factor }
		triple(4)
	"};
	check(src, "12");
}

#[test]
fn capture_undefined() {
	let src = indoc! {"
		f :: fn [missing] () int { 0 }
		f()
	"};
	fail_with(src, "undefined variable");
}

#[test]
fn capture_mut_writes_visible_outside() {
	let src = indoc! {"
		counter := 0
		inc :: fn [mut counter] () int { counter = counter + 1; counter }
		inc()
		inc()
		counter
	"};
	check(src, "2");
}

#[test]
fn capture_mut_requires_mut_binding() {
	let src = indoc! {"
		x :: 3
		f :: fn [mut x] () int { x }
		f()
	"};
	fail_with(src, "cannot capture `x` as `mut`");
}

#[test]
fn capturing_closure_rejected_as_plain_fn_param() {
	let src = indoc! {"
		apply :: fn(f: fn(int) int, x: int) int { f(x) }
		factor :: 2
		scale :: fn [factor] (n: int) int { n * factor }
		apply(scale, 21)
	"};
	fail_with(src, "wrong argument type");
}

#[test]
#[ignore]
// FIX: broke by sandwiches
fn implicit_capture_read_only() {
	let src = indoc! {"
		n :: 10
		scale :: fn (x: int) int { x * n }
		scale(5)
	"};
	check(src, "50");
}

#[test]
#[ignore]
// FIX: broke by sandwiches
fn implicit_capture_multiple() {
	let src = indoc! {"
		a :: 10
		b :: 32
		add :: fn () int { a + b }
		add()
	"};
	check(src, "42");
}

#[test]
fn implicit_capture_ignores_shadowed_inner_binding() {
	let src = indoc! {"
		n :: 10
		f :: fn () int { n :: 5; n }
		f() + n
	"};
	check(src, "15");
}

#[test]
fn implicit_capture_ignores_param_shadowing_outer() {
	let src = indoc! {"
		n :: 10
		f :: fn (n: int) int { n * 2 }
		f(4) + n
	"};
	check(src, "18");
}

#[test]
#[ignore]
// FIX: broke by sandwiches
fn implicit_capture_ignores_for_loop_pattern() {
	let src = indoc! {"
		nums :: [1, 2, 3]
		total := 0
		f :: fn () int {
			sum := 0
			loop n in nums { sum = sum + n }
			sum
		}
		total = f()
		total
	"};
	check(src, "6");
}

#[test]
fn closure_cannot_be_returned() {
	let src = indoc! {"
		make :: fn() {
			n :: 10
			return fn () int { n }
		}
		make()
	"};
	fail_with(src, "borrows its captures, so it can't be returned");
}

#[test]
fn closure_cannot_be_stored_in_array_literal() {
	let src = indoc! {"
		n :: 10
		arr :: [fn [n] () int { n }]
	"};
	fail_with(src, "borrows its captures, so it can't be stored in an array");
}

#[test]
fn closure_cannot_be_smuggled_through_a_generic_store() {
	let src = indoc! {"
		smuggle[T] :: fn(x: T) []T {
			a: []T
			a << x
			a
		}
		n :: 10
		smuggle(fn [n] () int { n })
	"};
	fail_with(src, "borrows its captures, so it can't be stored in an array");
}

#[test]
fn closure_cannot_be_a_map_value() {
	let src = indoc! {r#"
		n :: 10
		{ "a" = fn [n] () int { n } }
	"#};
	fail_with(src, "borrows its captures, so it can't be stored in a map");
}

#[test]
fn closure_cannot_be_stored_in_a_struct_field() {
	let src = indoc! {"
		Box[T] :: struct { v: T }
		n :: 10
		Box.{ v = fn [n] () int { n } }
	"};
	fail_with(src, "borrows its captures, so it can't be stored in a field");
}

#[test]
#[ignore]
// FIX: a capturing closure has no expressible return type
fn move_capture_escapes_via_return() {
	let src = indoc! {"
		make :: fn() fn() int {
			xs :: [7]
			return fn [move xs] () int { xs[0] }
		}
		f :: make()
		f()
	"};
	check(src, "7");
}

#[test]
fn move_capture_kills_the_name() {
	let src = indoc! {"
		xs :: [7]
		f :: fn [move xs] () int { xs[0] }
		print(xs)
	"};
	fail_with(src, "undefined variable");
}

#[test]
fn move_capture_of_fn_param_is_borrowed() {
	let src = indoc! {"
		make :: fn(xs: []int) fn() int {
			fn [move xs] () int { xs[0] }
		}
		make([1])
	"};
	fail_with(src, "cannot move `xs`, it is borrowed here");
}

#[test]
fn move_capture_inside_loop_of_outer_binding() {
	let src = indoc! {"
		xs :: [1]
		i := 0
		loop i < 2 {
			f :: fn [move xs] () int { xs[0] }
			i = i + 1
		}
	"};
	fail_with(src, "cannot move `xs` out of the enclosing loop");
}

#[test]
#[ignore]
// FIX: broke by sandwiches
fn implicit_capture_ignores_match_bound_name() {
	let src = indoc! {r#"
		r :: !int(7)
		f :: fn () int {
			match r {
				.ok(n) => n * 2,
				.err(e) => -1,
			}
		}
		f()
	"#};
	check(src, "14");
}

#[test]
fn trailing_block_infers_ret() {
	let src = indoc! {"
		retry :: fn(n: int, f: fn() int) int { f() + n }
		retry(2) { 21 }
	"};
	check(src, "23");
}

#[test]
fn trailing_fn_infers_ret() {
	let src = indoc! {"
		retry :: fn(n: int, f: fn() int) int { f() + n }
		retry(2) fn { 21 }
	"};
	check(src, "23");
}

#[test]
fn trailing_block_unit() {
	let src = indoc! {r#"
		run :: fn(f: fn() ()) { f() }
		run { print("hi") }
	"#};
	check(src, "hi");
}

#[test]
fn inline_arg_infers_ret() {
	let src = indoc! {"
		apply :: fn(f: fn() int) int { f() }
		apply(fn { 7 })
	"};
	check(src, "7");
}

#[test]
fn bare_fn_still_needs_ret_without_context() {
	let src = indoc! {"
		f := fn { 21 }
	"};
	fail_with(src, "explicit return type");
}

#[test]
fn param_inferred_from_fn_target() {
	let src = indoc! {"
		op :: fn(n: int, f: fn(int) int) int { f(n) }
		print(op(4, fn { $ * 4 }))
		print(op(4) { $ + 1 })
	"};
	check(src, ["16", "5"]);
}

#[test]
fn params_tuple_inferred() {
	let src = indoc! {r#"
		apply :: fn(f: fn(int, string) string) string { f(1, "x") }
		apply(fn { $.1 + "{$.0}" })
	"#};
	check(src, "x1");
}
