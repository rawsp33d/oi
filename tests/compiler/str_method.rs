use crate::helpers::*;

#[test]
fn primitives() {
	check("42.str()", "42");
	check("3.14.str()", "3.14");
	check("true.str()", "true");
	check(r#""hi".str()"#, "hi");
}

#[test]
fn composites() {
	check(r#"(1, "a").str()"#, r#"(1, "a")"#);
	check("[1, 2].str()", "[1, 2]");
}

#[test]
fn variants() {
	check("enum Color { Red, Green }\nColor.Red.str()", "Red");
	check("o := ?int(none)\no.str()", "none");
	check("r := !int(42)\nr.str()", "ok");
}

#[test]
fn derived_struct() {
	let src = indoc! {"
		struct Bag { items []int }
		Bag{[1, 2, 3]}.str()
	"};
	check(src, "Bag{items: [1, 2, 3]}");
}

#[test]
fn user_str_wins() {
	let src = indoc! {r#"
		struct Money { n int }
		impl Money { fn str(self) string { "$" } }
		Money{5}.str()
	"#};
	check(src, "$");
}
