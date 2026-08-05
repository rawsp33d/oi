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
		"struct Node { value int }\nmut n &Node",
		"a reference must be initialized (`?&T` for an optional one)",
	);
}

#[test]
fn optional_ref_zero_value_assign_unwrap() {
	check("struct Node { value int }\nmut o ?&Node\no", "none");
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
