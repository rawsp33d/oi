use crate::common::Project;
use crate::helpers::*;

#[test]
fn comp_folds_const_arithmetic() {
	check(["PI :: comp 22.0 / 7.0", "print(PI)"], "3.142857142857143");
}

#[test]
fn comp_calls_a_user_fn() {
	let src = indoc! {"
		f :: fn() int { 40 + 2 }
		V :: comp f()
		print(V)
	"};
	check(src, "42");
}

#[test]
fn comp_block_with_a_local() {
	let src = indoc! {"
		X :: comp {
			a := 10
			a * 2
		}
		print(X)
	"};
	check(src, "20");
}

#[test]
fn comp_str_result() {
	check(r#"S :: comp "hi" + " there"; print(S)"#, "hi there");
}

#[test]
fn comp_folds_structs() {
	let src = indoc! {r#"
		Point :: struct { x: int, y: int }
		Config :: struct { name: string, origin: Point }
		mk :: fn() Config { Config.{ "grid", Point.{ 3, 4 } } }
		C :: comp mk()
		print("{C.name} {C.origin.x} {C.origin.y}")
	"#};
	check(src, "grid 3 4");
}

#[test]
fn comp_calls_an_imported_fn() {
	let src = indoc! {"
		use math
		V :: comp math.abs(0 - 5)
		print(V)
	"};
	check(src, "5");
}

#[test]
fn comp_is_actually_comptime() {
	let src = indoc! {r#"
		print("run")
		V :: comp { print("fold") 7 }
		print(V)
	"#};
	check(src, ["fold", "run", "7"]);
}

#[test]
fn comp_if_is_conditional_compilation() {
	let src = indoc! {r#"
		log :: fn(msg: string) {
			comp if 1 == 1 { print(msg) } else { missing() }
		}
		comp if 2 < 1 { missing() }
		log("hi")
		print(comp if 1 == 2 { missing() } else if 2 == 2 { 42 })
	"#};
	check(src, ["hi", "42"]);
}

#[test]
fn comp_rejects_unreifiable_type() {
	fail_with("A :: comp [1, 2, 3]", "can't use this type in `comp` yet");
}

#[test]
fn comp_consts_in_a_module() {
	Project::new()
		.file("main.oi", ["module main", "use util", "print(util.BEST)"])
		.file(
			"util/lib.oi",
			[
				"module util",
				"pick :: fn() int { 40 + 2 }",
				"pub BEST :: comp SEED * 21",
				"pub SEED :: comp pick() / 21",
			],
		)
		.check("42");
}

#[test]
fn comp_folds_in_an_imported_module() {
	Project::new()
		.file("main.oi", ["module main", "use util", "print(util.f())"])
		.file(
			"util/lib.oi",
			["module util", r#"pub f :: fn() int { comp { print("fold") 40 + 2 } }"#],
		)
		.check(["fold", "42"]);
}
