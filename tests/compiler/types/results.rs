use crate::helpers::*;
use indoc::indoc;

#[test]
fn construct_ok() {
	check("!int(42)", "ok(42)");
}

#[test]
fn construct_err() {
	check(r#"!int(error("oops"))"#, r#"err("oops")"#);
}

#[test]
fn zero_value_is_ok() {
	check("mut r !int; r", "ok(0)");
}

#[test]
fn ord_gives_tag() {
	check("ord(!int(42))", "0");
	check(r#"ord(!int(error("oops")))"#, "1");
}

#[test]
fn int_cast_errors() {
	fail_with("int(!int(42))", "no backing value");
}

#[test]
fn eq_same_ok() {
	check("!int(42) == !int(42)", "true");
}

#[test]
fn eq_different_ok() {
	check("!int(42) == !int(7)", "false");
}

#[test]
fn eq_ok_vs_err() {
	check(r#"!int(42) == !int(error("oops"))"#, "false");
	check(r#"!int(42) != !int(error("oops"))"#, "true");
}

#[test]
fn field_type_mismatch() {
	fail_with("!int(3.0)", "expected int or Error, got float");
}

#[test]
fn ordering_rejected() {
	fail_with("!int(1) < !int(2)", "only `==`&`!=`");
}

#[test]
fn match_binds_ok() {
	check(
		indoc! {r#"
			r := !int(42)
			match r {
				.ok(n) => n,
				.err(e) => -1,
			}
		"#},
		"42",
	);
}

#[test]
fn match_err_arm() {
	check(
		indoc! {r#"
			r := !int(error("oops"))
			match r {
				.ok(n) => n,
				.err(e) => -1,
			}
		"#},
		"-1",
	);
}

#[test]
fn match_non_exhaustive_errors() {
	fail_with(
		indoc! {r"
			r := !int(42)
			match r {
				.ok(n) => n,
			}
		"},
		"non-exhaustive match, missing: err",
	);
}

#[test]
fn struct_field_type() {
	check(
		"struct Box { val !int }
		b := Box{ val: !int(42) }
		b.val",
		"ok(42)",
	);
}

#[test]
fn fn_param_type() {
	let src = indoc! {"
		fn unwrap_or(r !int, fallback int) int {
			match r {
				.ok(n) => n,
				.err(e) => fallback,
			}
		}
		unwrap_or(!int(42), 0)
	"};
	check(src, "42");
}

#[test]
fn bare_value_return_wraps_ok() {
	let src = indoc! {"
		fn find(x int) !int {
			return x
		}
		find(5)
	"};
	check(src, "ok(5)");
}

#[test]
fn bare_error_return_wraps_err() {
	let src = indoc! {r#"
		fn find(x int) !int {
			return error("not found")
		}
		find(5)
	"#};
	check(src, r#"err("not found")"#);
}

#[test]
fn error_message() {
	check(r#"error("oops").message()"#, "oops");
}

#[test]
fn error_message_via_dollar() {
	let src = indoc! {r#"
		!int(error("boom")) or {
			print($.message())
			0
		}
	"#};
	check(src, "boom\n0");
}

#[test]
fn error_unknown_method() {
	fail_with(r#"error("oops").code()"#, "`Error` has no method `code`");
}

#[test]
fn long_form_matches_shorthand() {
	let src = indoc! {r#"
		fn load(path string) Result[int, Error] {
			if path == "ok" { return 42 }
			return error("missing")
		}
		fn double(path string) Result[int, Error] {
			v := load(path)?
			v * 2
		}
		double("ok") or { -1 }
	"#};
	check(src, "84");
	let src = indoc! {r#"
		fn load(path string) Result[int, Error] {
			if path == "ok" { return 42 }
			return error("missing")
		}
		fn double(path string) Result[int, Error] {
			v := load(path)?
			v * 2
		}
		double("nope") or {
			print($)
			0
		}
	"#};
	check(src, ["missing", "0"]);
}

#[test]
fn long_form_nested() {
	let src = indoc! {r#"
		fn load() Result[[]int, Error] {
			return [1, 2, 3]
		}
		load() or { [-1] }
	"#};
	check(src, "[1, 2, 3]");
}

#[test]
fn long_form_rejects_custom_error() {
	fail_with(
		"fn load() Result[int, MyError] { 42 }; load()",
		"custom error types aren't supported yet",
	);
}
