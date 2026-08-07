use crate::helpers::{check, fail_with};

#[test]
fn pub_fn_runs() {
	check(r#"pub fn foo() { print("hi") } foo()"#, "hi");
}

#[test]
fn module_decl() {
	check(["module main", r#"print("ok")"#], "ok");
	fail_with("module other", "the entry file is module `main`");
	fail_with(["x := 1", "module main"], "`module` must come first");
}

#[test]
fn import_missing() {
	fail_with("import os", "cannot find module `os`");
}

#[test]
fn import_forms_not_supported_yet() {
	fail_with("import os { input }", "selective imports aren't supported yet");
}
