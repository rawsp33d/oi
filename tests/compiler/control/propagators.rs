use crate::helpers::*;

#[test]
fn question_unwraps_some() {
	let src = indoc! {"
		fn find(id: int) ?int {
			if id == 7 { return 42 }
			return none
		}
		fn display(id: int) ?int {
			v :: find(id)?
			v + 1
		}
		display(7) or { -1 }
	"};
	check(src, "43");
}

#[test]
fn question_propagates_none() {
	let src = indoc! {"
		fn find(id: int) ?int {
			if id == 7 { return 42 }
			return none
		}
		fn display(id: int) ?int {
			v :: find(id)?
			v + 1
		}
		display(1) or { -1 }
	"};
	check(src, "-1");
}

#[test]
fn bang_unwraps_ok() {
	let src = indoc! {r#"
		fn load(path: string) !int {
			if path == "ok" { return 42 }
			return error("missing")
		}
		fn double(path: string) !int {
			v :: load(path)?
			v * 2
		}
		double("ok") or { -1 }
	"#};
	check(src, "84");
}

#[test]
fn bang_propagates_error() {
	let src = indoc! {r#"
		fn load(path: string) !int {
			if path == "ok" { return 42 }
			return error("missing")
		}
		fn double(path: string) !int {
			v :: load(path)?
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
fn question_infers_enclosing_return_type() {
	let src = indoc! {"
		fn find(id: int) ?int {
			if id == 7 { return 42 }
			return none
		}
		fn display(id: int) {
			v :: find(id)?
			v + 1
		}
		display(7) or { -1 }
	"};
	check(src, "43");
}

#[test]
fn requires_option_or_result() {
	fail_with("fn f() int { 42? }\nf()", "`?` needs a `?T` or `!T` value");
}

#[test]
fn option_panics_in_main() {
	let src = indoc! {"
		fn find(id: int) ?int {
			if id == 7 { return 42 }
			return none
		}
		find(1)?
	"};
	fail_with(src, "panic: unwrapped `none`");
}

#[test]
fn result_panics_in_main() {
	let src = indoc! {r#"
		fn load(path: string) !int {
			if path == "ok" { return 42 }
			return error("missing")
		}
		load("nope")?
	"#};
	fail_with(src, "panic: missing");
}

#[test]
fn requires_matching_enclosing_return() {
	let src = indoc! {"
		fn find(id: int) ?int {
			if id == 7 { return 42 }
			return none
		}
		fn display(id: int) int {
			find(id)?
		}
		display(7)
	"};
	fail_with(src, "needs an enclosing fn returning `?T`");
}

#[test]
fn requires_matching_enclosing_return_result() {
	let src = indoc! {r#"
		fn load(path: string) !int {
			if path == "ok" { return 42 }
			return error("missing")
		}
		fn display(path: string) ?int {
			load(path)?
		}
		display("ok")
	"#};
	fail_with(src, "needs an enclosing fn returning `!T`");
}
