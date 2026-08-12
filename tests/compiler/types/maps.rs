use crate::helpers::*;
use indoc::indoc;

#[test]
fn declare_and_set_get() {
	check(
		indoc! {r#"
			m: Map[string, int]
			m["one"] = 1
			m["one"]
		"#},
		"1",
	);
}

#[test]
fn init_expr_declare_and_set_get() {
	check(
		indoc! {r#"
			m := Map[string, int]{}
			m["one"] = 1
			m["one"]
		"#},
		"1",
	);
}

#[test]
fn bare_map_lit_from_annotation() {
	check(
		indoc! {r#"
			m: Map[string, int] = Map{}
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
			m: Map[string, int]
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
			m: Map[string, int]
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
		indoc! {r#"
			m: Map[int, string]
			m[1] = "a"
			m[2] = "b"
			m[1]
		"#},
		"a",
	);
}

#[test]
fn float_keys() {
	check(
		indoc! {"
			m: Map[float, int]
			m[1.2] = 6
			m[2.1] = 9
			m[2.1]
		"},
		"9",
	);
}

#[test]
fn bool_keys() {
	check(
		indoc! {"
			m: Map[bool, int]
			m[true] = 6
			m[false] = 9
			m[false]
		"},
		"9",
	);
}

#[test]
fn tuple_keys_fail_for_now() {
	// TODO: actually implement complex keys and fix test
	assert!(
		fail(indoc! {"
			Point :: (int, int)
			m: Map[Point, int]
			m[(1, 2)] = 6
			m[(2, 1)] = 9
			m[(2, 1)]
		"})
		.contains("tuple cannot be used as a map key")
	);
}

#[test]
fn missing_key_panics() {
	assert!(
		fail(indoc! {r#"
			m: Map[string, int]
			m["missing"]
		"#})
		.contains("key not found")
	);
}

#[test]
fn wrong_key_type() {
	assert!(
		fail(indoc! {r#"
			m: Map[string, int]
			m[1]
		"#})
		.contains("expected str key")
	);
}

#[test]
fn wrong_value_type() {
	assert!(
		fail(indoc! {r#"
			m: Map[string, int]
			m["a"] = "b"
		"#})
		.contains("type mismatch")
	);
}

#[test]
fn record_literal_infers() {
	check(
		indoc! {r#"
			m :: { one = 1, two = 2 }
			m["one"] + m["two"]
		"#},
		"3",
	);
}

#[test]
fn record_literal_multiline() {
	check(
		indoc! {r#"
			m :: {
				one = 1
				two = 2
			}
			m["two"]
		"#},
		"2",
	);
}

#[test]
fn record_int_keys() {
	check(
		indoc! {r#"
			m :: { 1 = "one", 2 = "two" }
			m[1]
		"#},
		"one",
	);
}

#[test]
fn record_atom_keys() {
	check(
		indoc! {"
			m :: { :ok = 200, :not_found = 404 }
			m[:ok]
		"},
		"200",
	);
}

#[test]
fn record_typed_target() {
	check(
		indoc! {r#"
			m: Map[string, f64] : { a = 1.5 }
			m["a"]
		"#},
		"1.5",
	);
}

#[test]
fn record_empty_against_target() {
	check(
		indoc! {r#"
			m: Map[string, int] = {}
			m["a"] = 3
			m["a"]
		"#},
		"3",
	);
}

#[test]
fn record_pun() {
	check(
		indoc! {r#"
			x :: 5
			m :: {x,}
			m["x"]
		"#},
		"5",
	);
}

#[test]
fn record_mixed_value_types_fail() {
	fail_with(r#"m :: { a = 1, b = "two" }"#, "expected int, got str");
}

#[test]
fn map_prefix_lit() {
	check(
		indoc! {r#"
			num_map := Map{
				one = 1
				two = 2
			}
			num_map["one"]
		"#},
		"1",
	);
}

#[test]
fn record_empty_needs_target() {
	fail_with("m :: {}", "cannot infer");
}

#[test]
fn delete_key() {
	check(
		indoc! {r#"
			m: Map[string, int]
			m["one"] = 1
			m["two"] = 2
			m.delete["one"]
			m["two"]
		"#},
		"2",
	);
}

#[test]
fn delete_missing_key_is_noop() {
	check(
		indoc! {r#"
			m: Map[string, int]
			m.delete["missing"]
			1
		"#},
		"1",
	);
}

#[test]
fn deleted_key_then_lookup_panics() {
	fail_with(
		indoc! {r#"
			m: Map[string, int]
			m["one"] = 1
			m.delete["one"]
			m["one"]
		"#},
		"key not found",
	);
}

#[test]
fn init_with_elements_fails() {
	fail_with(r#"Map[string, int]{"x"}"#, "only array initializers take elements");
}

#[test]
fn delete_on_immutable_map_fails() {
	fail_with(
		indoc! {r#"
			f :: fn(m: Map[string, int]) int {
				m.delete["one"]
				m["one"]
			}
			n: Map[string, int]
			n["one"] = 1
			f(n)
		"#},
		"immutable",
	);
}

// value semantics (COW)

#[test]
fn index_assign_copy() {
	check(
		indoc! {r#"
			m: Map[string, int]
			m["a"] = 1
			b :: m
			m["a"] = 99
			b["a"]
		"#},
		"1",
	);
	check(
		indoc! {r#"
			m: Map[string, int]
			m["a"] = 1
			b := m
			b["a"] = 99
			m["a"]
		"#},
		"1",
	);
}

#[test]
fn independent_copies() {
	// delete copy
	check(
		indoc! {r#"
			m: Map[string, int]
			m["a"] = 1
			m["b"] = 2
			n := m
			n.delete["a"]
			m["a"]
		"#},
		"1",
	);
	// chain of copies
	check(
		indoc! {r#"
			m: Map[string, int]
			m["a"] = 1
			n :: m
			o := n
			o["a"] = 99
			n["a"]
		"#},
		"1",
	);
	// returned param vs arg
	check(
		indoc! {r#"
			id :: fn(m: Map[string, int]) Map[string, int] { m }
			a: Map[string, int]
			a["a"] = 1
			r := id(a)
			r["a"] = 99
			a["a"]
		"#},
		"1",
	);
	// stored array value
	check(
		indoc! {r#"
			m: Map[string, []int]
			arr := [1]
			m["a"] = arr
			arr << 2
			m["a"]
		"#},
		"[1]",
	);
}
