use indoc::indoc;

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
fn foreign_ptr_roundtrips() {
	Project::new()
		.file(
			"main.oi",
			["use cext", "p := cext.malloc(16)", "cext.free(p)", "print(1)"],
		)
		.file(
			"cext.oi",
			[
				"module cext",
				"pub malloc : fn(size: usize) ptr : foreign",
				"pub free : fn(p: ptr) : foreign",
			],
		)
		.check("1");
}

#[test]
fn foreign_writes_through_array_ptr() {
	Project::new()
		.file(
			"main.oi",
			[
				"use cext",
				"buf := [1, 2, 3]",
				"cext.memset(buf.ptr, 0, 4)",
				"print(buf)",
			],
		)
		.file(
			"cext.oi",
			["module cext", "pub memset : fn(p: ptr, c: int, n: usize) : foreign"],
		)
		.check("[0, 2, 3]");
}

#[test]
fn foreign_typed_read_copies_out() {
	Project::new()
		.file(
			"main.oi",
			[
				"use cext",
				"buf: []i32 = .[7, 8, 9]",
				"cext.memset(buf.ptr, 0, 4)",
				"print(buf.ptr.array[i32](3))",
			],
		)
		.file(
			"cext.oi",
			["module cext", "pub memset : fn(p: ptr, c: int, n: usize) : foreign"],
		)
		.check("[0, 8, 9]");
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
fn c_struct_writes_at_c_offsets() {
	let src = indoc! {"
		@c
		S :: struct { a: u8, ok: bool, b: u32 }
		buf: []u8 = .[9, 9, 9, 9, 9, 9, 9, 9]
		buf.ptr.write(S.{ a = 1, ok = true, b = 2 })
		print(buf)
	"};
	check(src, "[1, 1, 9, 9, 2, 0, 0, 0]");
}

#[test]
fn c_struct_roundtrips_nested() {
	let src = indoc! {"
		@c Inner :: struct { x: u16, y: u16 }
		@c Outer :: struct { tag: u8, on: bool, inner: Inner }
		buf: []u8 = .[0, 0, 0, 0, 0, 0]
		buf.ptr.write(Outer.{ tag = 7, on = true, inner = Inner.{ x = 1, y = 2 } })
		print(buf)
		o := buf.ptr.read[Outer]()
		print(o.on)
		print(o.inner.y)
	"};
	check(src, ["[7, 1, 1, 0, 2, 0]", "true", "2"]);
}

#[test]
fn c_struct_rejects_missing_c_repr() {
	fail_with(
		["@c Bad :: struct { s: string }", "print(1)"],
		"`Bad.s` has no C representation",
	);
	let src = indoc! {"
		P :: struct { x: int }
		buf: []u8 = .[0]
		buf.ptr.write(P.{ x = 1 })
	"};
	fail_with(src, "`P` has no C layout");
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
