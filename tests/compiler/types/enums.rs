use crate::helpers::*;

#[test]
fn qualified_access() {
	check("enum Color { red green blue }\nColor.red", "red");
	check("enum Color { red green blue }\nColor.blue", "blue");
}

#[test]
fn oneliner() {
	check("enum Fruit { apple orange grape }\nFruit.orange", "orange");
}

#[test]
fn bind() {
	check(
		"enum Color { red green blue }\nc := Color.green\nc",
		"green",
	);
}

#[test]
fn reassign() {
	check(
		"enum Color { red green blue }\nmut c := Color.red\nc = Color.blue\nc",
		"blue",
	);
}

#[test]
fn first_variant_is_default() {
	check("enum Color { red green blue }\nmut c Color\nc", "red");
}

#[test]
fn eq_same() {
	check(
		"enum Color { red green blue }\nColor.red == Color.red",
		"true",
	);
}

#[test]
fn eq_different() {
	check(
		"enum Color { red green blue }\nColor.red == Color.blue",
		"false",
	);
}

#[test]
fn ne() {
	check(
		"enum Color { red green blue }\nColor.red != Color.blue",
		"true",
	);
}

#[test]
fn returned_from_fn() {
	check(
		"enum Color { red green blue }\nfn fav() Color { Color.blue }\nfav()",
		"blue",
	);
}

#[test]
fn struct_field() {
	check(
		"enum Stat { health mana stamina }
		struct User { s Stat }
		u := User{ s: Stat.mana }
		u.s",
		"mana",
	);
}

#[test]
fn in_match() {
	check(
		indoc! {r#"
			enum Color { red green blue }
			c := Color.green
			match c {
				Color.red { "r" }
				Color.green { "g" }
				else { "?" }
			}
		"#},
		"g",
	);
}

#[test]
fn unknown_variant() {
	let err = fail("enum Color { red green blue }\nColor.purple");
	assert!(err.contains("no variant `purple`"), "got: {err}");
}
