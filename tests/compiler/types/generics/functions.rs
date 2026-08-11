use crate::helpers::*;
use indoc::indoc;

#[test]
fn max_int() {
	let src = indoc! {"
		fn max[T](a: T, b: T) T {
			if a > b { a } else { b }
		}
		max(3, 7)
	"};
	check(src, "7");
}

#[test]
fn max_float_instantiation_is_independent() {
	let src = indoc! {"
		fn max[T](a: T, b: T) T {
			if a > b { a } else { b }
		}
		a := max(3, 7)
		max(3.5, 1.2)
	"};
	check(src, "3.5");
}

#[test]
fn self_recursive_generic() {
	let src = indoc! {"
		fn fact[T](n: T) T {
			if n <= 1 { 1 } else { n * fact(n - 1) }
		}
		fact(5)
	"};
	check(src, "120");
}

#[test]
fn mutually_recursive_generics() {
	let src = indoc! {"
		fn is_even[T](n: T) bool {
			if n == 0 { true } else { is_odd(n - 1) }
		}
		fn is_odd[T](n: T) bool {
			if n == 0 { false } else { is_even(n - 1) }
		}
		is_even(10)
	"};
	check(src, "true");
}

#[test]
fn first_of_array() {
	let src = indoc! {"
		fn first[T](xs: []T) ?T {
			if xs.len == 0 { ?T(none) } else { ?T(xs[0]) }
		}
		first([1, 2, 3])
	"};
	check(src, "some(1)");
}

#[test]
fn type_mismatch_across_args() {
	fail_with(
		indoc! {r#"
			fn max[T](a: T, b: T) T { if a > b { a } else { b } }
			max(1, "a")
		"#},
		"bound to both",
	);
}

#[test]
fn missing_return_type_errors() {
	fail_with(
		indoc! {r"
			fn noret[T](x: T) { x }
			noret(1)
		"},
		"needs an explicit return type",
	);
}

#[test]
fn explicit_type_arg_when_uninferable() {
	let src = indoc! {"
		fn none_of[T]() ?T {
			?T(none)
		}
		none_of[int]()
	"};
	check(src, "none");
}

#[test]
fn explicit_type_arg_redundant_with_inference() {
	let src = indoc! {"
		fn max[T](a: T, b: T) T {
			if a > b { a } else { b }
		}
		max[int](3, 7)
	"};
	check(src, "7");
}

#[test]
fn explicit_type_arg_conflicts_with_args() {
	fail_with(
		indoc! {r#"
			fn max[T](a: T, b: T) T { if a > b { a } else { b } }
			max[int](3, "a")
		"#},
		"bound to both",
	);
}

#[test]
fn explicit_type_arg_count_mismatch() {
	fail_with(
		indoc! {"
			fn max[T](a: T, b: T) T { if a > b { a } else { b } }
			max[int, string](3, 7)
		"},
		"expects 1 type argument",
	);
}

#[test]
fn explicit_type_arg_on_non_generic_errors() {
	fail_with(
		indoc! {"
			fn add(a: int, b: int) int { a + b }
			add[int](3, 7)
		"},
		"is not generic",
	);
}

#[test]
fn bounded_type_param_parses_and_runs() {
	let src = indoc! {"
		fn biggest[T: Ord](a: T, b: T) T {
			if a > b { a } else { b }
		}
		biggest(3, 7)
	"};
	check(src, "7");
}

#[test]
fn bounded_type_param_with_explicit_type_arg() {
	let src = indoc! {"
		fn biggest[T: Ord](a: T, b: T) T {
			if a > b { a } else { b }
		}
		biggest[int](3, 7)
	"};
	check(src, "7");
}
