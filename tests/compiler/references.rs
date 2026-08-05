use crate::helpers::*;

#[test]
fn aliasing_shares_identity() {
	check(
		indoc! {r#"
			struct Node { value int }
			mut list := &Node{ value: 5 }
			head := list
			list.value = 9
			print("head is {head}")
		"#},
		"head is Node{value: 9}",
	);
}

#[test]
fn bare_ref_without_value_errors() {
	fail_with(
		["struct Node { value int }", "mut n &Node"],
		"a reference must be initialized (`?&T` for an optional one)",
	);
}

#[test]
fn optional_ref_zero_value_assign_unwrap() {
	check(["struct Node { value int }", "mut o ?&Node", "o"], "none");
	check(
		indoc! {"
			struct Node { value int }
			mut o ?&Node
			o = ?&Node(&Node{ value: 7 })
			match o {
				.some(n) => n.value,
				.none => -1,
			}
		"},
		"7",
	);
}

#[test]
fn no_leaks_on_share_and_release() {
	assert_clean(indoc! {"
		struct Node { value int }
		mut list := &Node{ value: 5 }
		head := list
		list.value = 9
		print(head.value)
	"});
}

#[test]
fn bare_ref_field_rejected() {
	let src = indoc! {"
		struct Node { value int }
		struct List { head &Node }
	"};
	fail_with(src, "must be optional (`?&T`)");
}

#[test]
fn linked_nodes() {
	check(
		indoc! {r#"
			struct Node { value int, next ?&Node }
			tail := &Node{ value: 2 }
			head := &Node{ value: 1, next: ?&Node(tail) }
			match head.next {
				.some(n) => print("{head.value} -> {n.value}"),
				.none => print("lonely"),
			}
		"#},
		"1 -> 2",
	);
}

#[test]
fn interior_ref_frees_on_release() {
	assert_clean(indoc! {"
		struct Node { value int, next ?&Node }
		head := &Node{ value: 1, next: ?&Node(&Node{ value: 2 }) }
		print(head.value)
	"});
}

#[test]
fn shared_option_ref_stays_clean() {
	assert_clean(indoc! {"
		struct Node { value int, next ?&Node }
		t := &Node{ value: 2 }
		o := ?&Node(t)
		a := &Node{ value: 1, next: o }
		b := &Node{ value: 3, next: o }
		print(t.value)
	"});
}

#[test]
fn returned_box_keeps_zeroed_field() {
	let src = indoc! {r#"
		struct Node { value int, next ?&Node }
		fn make() &Node { &Node{ value: 1 } }
		n := make()
		match n.next {
			.some(x) => print(x.value),
			.none => print("ok"),
		}
	"#};
	check(src, "ok");
	assert_clean(src);
}

#[test]
fn rebind_releases_old_target() {
	assert_clean(indoc! {r#"
		struct Node { value int }
		mut o ?&Node
		o = ?&Node(&Node{ value: 1 })
		o = ?&Node(&Node{ value: 2 })
		print("done")
	"#});
}

#[test]
fn user_enum_with_ref_payload_stays_boxed() {
	check(
		indoc! {r#"
			struct Node { value int }
			enum E { empty, full(&Node) }
			e := E.full(&Node{ value: 7 })
			print(e)
			match e {
				.full(n) => print(n.value),
				.empty => print("no"),
			}
		"#},
		["full(Node{value: 7})", "7"],
	);
}

#[test]
fn value_recursion_still_errors() {
	fail_with(
		["struct A { b B }", "struct B { a A }"],
		"would require infinitely nested fields",
	);
}
