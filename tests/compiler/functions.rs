use crate::helpers::*;

#[test]
fn fn_call() {
	let src = indoc! {"
		double :: fn() { 21 * 2 }
		double()
	"};
	check(src, "42");
}

#[test]
fn multi_fn() {
	let src = indoc! {"
		base :: fn() {
			6
		}

		triple :: fn() {
			base() + base() + base()
		}

		triple()
	"};
	check(src, "18");
}

#[test]
fn fn_vars() {
	let src = indoc! {"
		area :: fn() {
			width :: 12
			height :: 5
			width * height
		}

		area()
	"};
	check(src, "60");
}

#[test]
fn fn_args() {
	let src = indoc! {"
		add :: fn(x: int, y: int) {
			x + y
		}
		add(3, 4)
	"};
	check(src, "7");
}

#[test]
fn fn_arg_passthrough() {
	let src = indoc! {"
		identity :: fn(x: int) { x }
		identity(99)
	"};
	check(src, "99");
}

#[test]
fn fn_args_nested() {
	let src = indoc! {"
		add :: fn(x: int, y: int) { x + y }
		add3 :: fn(a: int, b: int, c: int) { add(add(a, b), c) }
		add3(1, 2, 3)
	"};
	check(src, "6");
}

#[test]
fn fn_arg_float() {
	let src = indoc! {"
		scale :: fn(x: f64) { x * 2.0 }
		scale(2.5)
	"};
	check(src, "5.0");
}

#[test]
fn fn_arg_trailing_comma() {
	let src = indoc! {"
		add :: fn(x: int, y: int,) { x + y }
		add(40, 2,)
	"};
	check(src, "42");
}

#[test]
fn self_recursion() {
	let src = indoc! {"
		fact :: fn(n: int) int { if n <= 1 { 1 } else { n * fact(n - 1) } }
		fact(5)
	"};
	check(src, "120");
}

#[test]
fn forward_reference() {
	let src = indoc! {"
		a :: fn() int { b() + 1 }
		b :: fn() int { 41 }
		a()
	"};
	check(src, "42");
}

#[test]
fn fn_arg_wrong_type() {
	let src = indoc! {"
		i :: fn(x: int) { x }
		i(2.4)
	"};
	fail_with(src, "wrong argument type");
}

#[test]
fn fn_return_type() {
	let src = indoc! {"
		add :: fn(x: int, y: int) int {
			x + y
		}
		add(3, 4)
	"};
	check(src, "7");
}

#[test]
fn fn_return_type_float() {
	let src = indoc! {"
		scale :: fn(x: f64) f64 { x * 2.0 }
		scale(2.5)
	"};
	check(src, "5.0");
}

#[test]
fn fn_return_keyword() {
	let src = indoc! {"
		add :: fn(x: int, y: int) int {
			return x + y
		}
		add(3, 4)
	"};
	check(src, "7");
}

#[test]
fn fn_return_short_circuits() {
	let src = indoc! {"
		five :: fn() int {
			return 5
			10
		}
		five()
	"};
	check(src, "5");
}

#[test]
fn fn_return_bare() {
	// a bare `return` yields the zero value of the return type
	let src = indoc! {"
		z :: fn() int { return }
		z()
	"};
	check(src, "0");
}
