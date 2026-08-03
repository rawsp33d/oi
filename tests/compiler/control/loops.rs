use crate::helpers::*;

#[test]
fn counts_to_a_return() {
	let src = indoc! {"
		mut i := 0
		loop {
			i = i + 1
			if i == 3 { return i }
		}
	"};
	check(src, "3");
}

#[test]
fn sums_across_iterations() {
	let src = indoc! {"
		mut sum := 0
		mut i := 0
		loop {
			i = i + 1
			sum = sum + i
			if i == 4 { return sum }
		}
	"};
	check(src, "10");
}

#[test]
fn loop_in_function() {
	let src = indoc! {"
		fn pow2_over(n int) int {
			mut x := 1
			loop {
				if x > n { return x }
				x = x * 2
			}
		}
		pow2_over(10)
	"};
	check(src, "16");
}

#[test]
fn break_exits() {
	let src = indoc! {"
		mut i := 0
		loop {
			i = i + 1
			if i == 3 { break }
		}
		i
	"};
	check(src, "3");
}

#[test]
fn continue_skips() {
	let src = indoc! {"
		mut sum := 0
		mut i := 0
		loop {
			i = i + 1
			if i > 10 { break }
			if i % 2 == 1 { continue }
			sum = sum + i
		}
		sum
	"};
	check(src, "30");
}

#[test]
fn break_targets_innermost() {
	let src = indoc! {"
		mut outer := 0
		loop {
			outer = outer + 1
			mut inner := 0
			loop {
				inner = inner + 1
				if inner == 2 { break }
			}
			if outer == 3 { break }
		}
		outer
	"};
	check(src, "3");
}

#[test]
fn break_outside_loop() {
	fail_with("break", "outside of a loop");
}

#[test]
fn continue_outside_loop() {
	fail_with("continue", "outside of a loop");
}

#[test]
fn while_counts() {
	let src = indoc! {"
		mut i := 0
		loop i < 5 {
			i = i + 1
		}
		i
	"};
	check(src, "5");
}

#[test]
fn while_never_enters() {
	let src = indoc! {"
		mut i := 10
		loop i < 5 {
			i = i + 1
		}
		i
	"};
	check(src, "10");
}

#[test]
fn while_sums() {
	let src = indoc! {"
		mut sum := 0
		mut i := 0
		loop i < 5 {
			i = i + 1
			sum = sum + i
		}
		sum
	"};
	check(src, "15");
}

#[test]
fn while_break() {
	let src = indoc! {"
		mut i := 0
		loop i < 100 {
			i = i + 1
			if i == 3 { break }
		}
		i
	"};
	check(src, "3");
}

#[test]
fn while_continue() {
	let src = indoc! {"
		mut sum := 0
		mut i := 0
		loop i < 10 {
			i = i + 1
			if i % 2 == 1 { continue }
			sum = sum + i
		}
		sum
	"};
	check(src, "30");
}

#[test]
fn while_condition_must_be_bool() {
	fail_with("loop 3 { }", "must be Bool");
}

// loops over ranges

#[test]
fn for_range_sums() {
	let src = indoc! {"
		mut sum := 0
		loop i in 0..5 {
			sum = sum + i
		}
		sum
	"};
	check(src, "10");
}

#[test]
fn for_range_excludes_end() {
	let src = indoc! {"
		loop i in 0..3 { print(i) }
	"};
	check(src, "0\n1\n2");
}

#[test]
fn for_range_empty() {
	let src = indoc! {"
		mut sum := 99
		loop i in 3..3 {
			sum = 0
		}
		sum
	"};
	check(src, "99");
}

#[test]
fn for_range_variable_bounds() {
	let src = indoc! {"
		lo := 2
		hi := 5
		mut sum := 0
		loop i in lo..hi {
			sum = sum + i
		}
		sum
	"};
	check(src, "9");
}

#[test]
fn for_range_break() {
	let src = indoc! {"
		mut sum := 0
		loop i in 0..100 {
			if i == 5 { break }
			sum = sum + i
		}
		sum
	"};
	check(src, "10");
}

#[test]
fn for_range_continue_advances() {
	let src = indoc! {"
		mut sum := 0
		loop i in 0..6 {
			if i % 2 == 1 { continue }
			sum = sum + i
		}
		sum
	"};
	check(src, "6");
}

#[test]
fn for_range_nested() {
	let src = indoc! {"
		mut n := 0
		loop i in 0..3 {
			loop j in 0..3 {
				n = n + 1
			}
		}
		n
	"};
	check(src, "9");
}

#[test]
fn for_range_returns() {
	let src = indoc! {"
		fn square_at(n int) int {
			loop i in 0..10 {
				if i == n { return i * i }
			}
			return 0
		}
		square_at(3)
	"};
	check(src, "9");
}

#[test]
fn for_var_is_scoped() {
	fail_with("loop i in 0..3 { i }\ni", "undefined variable");
}

// loops over iterables

#[test]
fn for_each_sums() {
	let src = indoc! {"
		mut sum := 0
		loop x in [2, 4, 6, 8] {
			sum = sum + x
		}
		sum
	"};
	check(src, "20");
}

#[test]
fn for_each_variable_array() {
	let src = indoc! {"
		a := [10, 20, 30]
		mut sum := 0
		loop x in a {
			sum = sum + x
		}
		sum
	"};
	check(src, "60");
}

#[test]
fn for_each_strings() {
	let src = indoc! {r#"
		loop s in ["a", "b", "c"] { write(s) }
		""
	"#};
	check(src, "abc");
}

#[test]
fn slice_iterates_its_window() {
	let src = indoc! {"
		a := [0, 2, 4, 6, 8]
		mut sum := 0
		loop x in a[1..4] {
			sum = sum + x
		}
		sum
	"};
	check(src, "12");
}

#[test]
fn for_each_tuple_destructure() {
	let src = indoc! {"
		mut sum := 0
		loop (x, y) in [(0, 0), (1, 2), (3, 4)] {
			sum = sum + x + y
		}
		sum
	"};
	check(src, "10");
}

#[test]
fn for_each_iterable_must_be_array() {
	fail_with("loop x in 5 { x }", "not iterable");
}

#[test]
fn for_range_bound_must_be_int() {
	fail_with("loop i in 0..true { i }", "must be Int");
}

#[test]
fn for_tuple_pattern_on_non_tuple() {
	fail_with("loop (x, y) in [1, 2, 3] { x }", "destructure");
}

#[test]
fn for_tuple_pattern_wrong_field_count() {
	fail_with("loop (x, y, z) in [(1, 2)] { x }", "fields");
}

#[test]
fn for_each_bind_is_independent_copy() {
	let src = indoc! {"
		outer := [[1], [2]]
		mut got := [0]
		loop x in outer {
			got = x
		}
		got << 99
		outer[1]
	"};
	check(src, "[2]");
}
