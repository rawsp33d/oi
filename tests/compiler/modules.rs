use crate::common::Project;
use crate::helpers::{check, fail_with};

#[test]
fn pub_fn_runs() {
	check(r#"pub foo :: fn() { print("hi") } foo()"#, "hi");
}

#[test]
fn module_decl() {
	check(["module main", r#"print("ok")"#], "ok");
	fail_with("module other", "the entry file is module `main`");
	fail_with(["x :: 1", "module main"], "`module` must come first");
}

#[test]
fn import_missing() {
	fail_with("use os", "cannot find module `os`");
}

#[test]
fn import_std() {
	check(["use core", r#"print("ok")"#], "ok");
}

#[test]
fn import_trait() {
	check(["use core.{ Ord }", "print(int is Ord)"], "true");
	check(["use core.{ Order :: Ord }", "print(int is Order)"], "true");
}

#[test]
fn import_nested_path() {
	fail_with("use a.b.c", "nested module paths aren't supported yet");
	fail_with("x :: use a.b.{ c }", "nested module paths aren't supported yet");
}

#[test]
fn rt_is_internal_to_core() {
	fail_with(["use rt", "print(1)"], "internal to core");
}

#[test]
fn foreign_outside_module_scope_fails() {
	fail_with(
		["x : fn() int : foreign", "print(1)"],
		"only allowed as a module-level binding",
	);
}

#[test]
fn foreign_resolves_process_symbols() {
	Project::new()
		.file("main.oi", ["use cext", "print(cext.abs(-5))"])
		.file("cext.oi", ["module cext", "pub abs : fn(x: i32) i32 : foreign"])
		.check("5");
}

#[test]
fn foreign_cstr_param_calls_strlen() {
	Project::new()
		.file("main.oi", ["use cext", r#"print(cext.strlen("hi!".cstr()))"#])
		.file("cext.oi", ["module cext", "pub strlen : fn(s: cstr) usize : foreign"])
		.check("3");
}

#[test]
fn foreign_unknown_symbol_fails() {
	Project::new()
		.file("main.oi", ["use cext", "print(1)"])
		.file(
			"cext.oi",
			["module cext", "zzz_definitely_not_a_symbol : fn() int : foreign"],
		)
		.fail_with("unknown foreign symbol");
}

#[test]
fn link_dlopens_named_library() {
	Project::new()
		.file("main.oi", ["use cext", "print(cext.zlibVersion().str()[0..1])"])
		.file(
			"cext.oi",
			["module cext", r#"@link.{"z"}"#, "pub zlibVersion : fn() cstr : foreign"],
		)
		.check("1");
}

#[test]
fn const_exprs() {
	Project::new()
		.file(
			"main.oi",
			["module main", "use util", "print(util.half)", "print(util.low)"],
		)
		.file(
			"util/lib.oi",
			["module util", "pub half :: 10 / 2", "pub low :: -2147483647 - 1"],
		)
		.check(["5", "-2147483648"]);
}
