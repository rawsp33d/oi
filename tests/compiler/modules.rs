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
