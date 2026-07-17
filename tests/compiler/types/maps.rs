use crate::helpers::*;
use indoc::indoc;

#[test]
fn declare_and_set_get() {
	check(
		indoc! {r#"
			mut m Map[string, int]
			m["one"] = 1
			m["one"]
		"#},
		"1",
	);
}

#[test]
fn overwrite_key() {
	check(
		indoc! {r#"
			mut m Map[string, int]
			m["a"] = 1
			m["a"] = 2
			m["a"]
		"#},
		"2",
	);
}

#[test]
fn multiple_keys() {
	check(
		indoc! {r#"
			mut m Map[string, int]
			m["one"] = 1
			m["two"] = 2
			m["one"] + m["two"]
		"#},
		"3",
	);
}

#[test]
fn int_keys() {
	check(
		indoc! {"
			mut m Map[int, string]
			m[1] = \"a\"
			m[2] = \"b\"
			m[1]
		"},
		"a",
	);
}

#[test]
fn missing_key_panics() {
	assert!(
		fail(indoc! {r#"
		mut m Map[string, int]
		m["missing"]
	"#})
		.contains("key not found")
	);
}

#[test]
fn wrong_key_type() {
	assert!(
		fail(indoc! {r#"
		mut m Map[string, int]
		m[1]
	"#})
		.contains("expected str key")
	);
}

#[test]
fn wrong_value_type() {
	assert!(
		fail(indoc! {r#"
		mut m Map[string, int]
		m["a"] = "b"
	"#})
		.contains("type mismatch")
	);
}
