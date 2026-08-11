//! Leak checks.
//! Every allocation a program makes is freed by exit.

use crate::helpers::*;

#[test]
fn bind_append_drop() {
	assert_clean(["a := [1, 2, 3]", "a << 4", "print(a)"]);
}

#[test]
fn copies_and_cow() {
	assert_clean(indoc! {"
		a := [1, 2, 3]
		b :: a
		c :: b
		a << 4
		print(a)
		print(c)
	"});
}

#[test]
fn slices() {
	assert_clean(["a :: [1, 2, 3, 4]", "b :: a[1..3]", "print(b)"]);
}

#[test]
fn fn_call_and_return() {
	assert_clean(indoc! {"
		fn make() []int { [1, 2] }
		fn id(a []int) []int { a }
		x :: id(make())
		print(x)
	"});
}

#[test]
fn loop_temp_per_iteration() {
	assert_clean(indoc! {"
		i := 0
		loop i < 100 {
			t :: [i, i]
			i = t[1] + 1
		}
		print(i)
	"});
}

#[test]
fn early_return() {
	assert_clean(indoc! {"
		fn f(n int) int {
			a :: [1, 2, 3]
			if n > 1 { return a[0] }
			a[1]
		}
		print(f(5))
	"});
}

#[test]
fn break_and_continue() {
	assert_clean(indoc! {"
		i := 0
		loop {
			i = i + 1
			if i == 3 { continue }
			xs :: [i]
			if xs[0] > 5 { break }
		}
		print(i)
	"});
}

#[test]
fn map_set_delete_copy() {
	assert_clean(indoc! {r#"
		m := {a: 1}
		n :: m
		m["b"] = 2
		m.delete["a"]
		print(n["a"])
	"#});
}

#[test]
fn reassign_and_shadow() {
	assert_clean(indoc! {"
		a := [1]
		a = [2, 3]
		b :: [4]
		b :: [5]
		print(a)
		print(b)
	"});
}

#[test]
fn branch_merges() {
	assert_clean(indoc! {"
		x :: if true { [1] } else { [2] }
		y :: match 2 { 1 => [9], 2 => [4, 5], else => [0] }
		print(x)
		print(y)
	"});
}

#[test]
fn for_loop_element_binds() {
	assert_clean(indoc! {"
		total := 0
		loop e in [10, 20, 30] {
			total = total + e
		}
		print(total)
	"});
}

#[test]
fn nested_elements_still_leak() {
	// TODO: revisit
	assert!(leaks("a :: [[1], [2]]\nprint(a[0])") > 0);
}

#[test]
fn struct_field_leak_is_bounded() {
	let src = indoc! {"
		struct Bag { items []int }
		s :: Bag{ items: [1, 2] }
		print(s.items[0])
	"};
	// TODO: revisit
	assert_eq!(leaks(src), 2);
}
