use crate::helpers::*;

#[test]
fn bind_basic() {
	check(r#"(foo, bar) :: ("food", "bard"); foo"#, "food");
	check(r#"(foo, bar) :: ("food", "bard"); bar"#, "bard");
}

#[test]
fn bind_from_fn() {
	let src = indoc! {"
		pair :: fn() (int, int) { (10, 20) }
		(a, b) :: pair()
		a + b
	"};
	check(src, "30");
}

#[test]
fn bind_mut_reassigned() {
	let src = indoc! {"
		(a, b) := (1, 2)
		a = a + b
		a
	"};
	check(src, "3");
}

#[test]
fn swap() {
	let src = indoc! {"
		(a, b) := (1, 2)
		(a, b) = (b, a)
		a
	"};
	check(src, "2");
}

#[test]
fn loose_commas() {
	let src = indoc! {r#"
		get_coords :: fn() (int, int) { (7, 2) }
		(lat long) :: get_coords()
		(a, b) := (lat long)
		(a b) = (b a)
		loop (x y) in [(1 2)] { print(x + y) }
		match (a b) { (l r) => print(l - r), }
	"#};
	check(src, "3\n-5");
}

#[test]
fn bare_tuple_still_expr() {
	check("(a, b) :: (1, 2)\n(a, b)", "(1, 2)");
}

#[test]
fn fail_arity_mismatch() {
	fail_with("(a, b, c) :: (1, 2)", "fields");
}

#[test]
fn fail_non_tuple() {
	fail_with("(a, b) :: 5", "expected a tuple");
}

#[test]
fn fail_assign_immutable() {
	fail_with("(a, b) :: (1, 2)\n(a, b) = (3, 4)", "immutably bound");
}
