use std::process::Output;

use crate::common::{Project, Run, err, oi, ok};

// Run main.oi in a project.
fn run_main(p: Project) -> Output {
	oi(&["run", "main.oi"]).current_dir(&p).run(None)
}

#[test]
fn imports_work() {
	let p = Project::new()
		.file("main.oi", ["module main", "use foo", "print(foo.total())"])
		.file(
			"foo/lib.oi",
			[
				"module foo",
				"use bar",
				"P :: struct { x: int, y: int }",
				"sum :: fn(p: P) int { p.x + p.y }",
				"pub total :: fn() int { bar.twice(sum(P.{x = 2, y = 5})) }",
			],
		)
		.file("bar/lib.oi", ["module bar", "pub twice :: fn(n: int) int { n * 2 }"]);
	assert_eq!(ok(run_main(p)), "14");
}

#[test]
fn private_fn_rejected() {
	let p = Project::new()
		.file("main.oi", ["module main", "use foo", "print(foo.secret())"])
		.file("foo/lib.oi", ["module foo", "secret :: fn() int { 1 }"]);
	let out = err(run_main(p));
	assert!(out.contains("private to module `foo`"), "{out}");
}

#[test]
fn wrong_module_decl_names_the_file() {
	let p = Project::new()
		.file("main.oi", ["module main", "use foo", "print(foo.hi())"])
		.file("foo/lib.oi", ["module bar", "pub hi :: fn() int { 1 }"]);
	let out = err(run_main(p));
	assert!(out.contains("foo/lib.oi"), "{out}");
	assert!(out.contains("module foo"), "{out}");
}

#[test]
fn import_cycle_rejected() {
	let p = Project::new()
		.file("main.oi", ["module main", "use a", "print(a.v())"])
		.file("a/m.oi", ["module a", "use b", "pub v :: fn() int { b.v() }"])
		.file("b/m.oi", ["module b", "use a", "pub v :: fn() int { a.v() }"]);
	let out = err(run_main(p));
	assert!(out.contains("import cycle"), "{out}");
}

#[test]
fn duplicate_name_across_files_rejected() {
	let p = Project::new()
		.file("main.oi", ["module main", "use foo", "print(foo.hi())"])
		.file("foo/a.oi", ["module foo", "pub hi :: fn() int { 1 }"])
		.file("foo/b.oi", ["module foo", "hi :: fn() int { 2 }"]);
	let out = err(run_main(p));
	assert!(out.contains("defined twice"), "{out}");
}

#[test]
fn import_alias() {
	let p = Project::new()
		.file("main.oi", ["module main", "f :: use foo", "print(f.hi())"])
		.file("foo/lib.oi", ["module foo", "pub hi :: fn() int { 7 }"]);
	assert_eq!(ok(run_main(p)), "7");

	let p = Project::new()
		.file("main.oi", ["module main", "f :: use foo", "print(foo.hi())"])
		.file("foo/lib.oi", ["module foo", "pub hi :: fn() int { 7 }"]);
	let out = err(run_main(p));
	assert!(out.contains("foo"), "{out}");
}

#[test]
fn selective_import() {
	for main in [
		"use foo\nuse foo.{ hi }\nprint(hi() + foo.hi())",
		"use foo.hi\nprint(hi() + hi())",
		"use foo.{ h :: hi }\nprint(h() + h())",
	] {
		let p = Project::new()
			.file("main.oi", ["module main", main])
			.file("foo/lib.oi", ["module foo", "pub hi :: fn() int { 7 }"]);
		assert_eq!(ok(run_main(p)), "14");
	}
}

#[test]
fn selective_import_fails() {
	let p = Project::new()
		.file("main.oi", ["module main", "use foo.{ nope }", "print(nope())"])
		.file("foo/lib.oi", ["module foo", "pub hi :: fn() int { 7 }"]);
	let out = err(run_main(p));
	assert!(out.contains("has no `nope`"), "{out}");

	let p = Project::new()
		.file("main.oi", ["module main", "use foo.{ secret }", "print(secret())"])
		.file("foo/lib.oi", ["module foo", "secret :: fn() int { 1 }"]);
	let out = err(run_main(p));
	assert!(out.contains("private to module `foo`"), "{out}");
}

#[test]
fn type_import() {
	for (main, lib) in [
		(
			"use foo.{ P }\np := P.{ x = 3, y = 4 }\nprint(p.x + p.y)",
			"pub P :: struct { x: int, y: int }",
		),
		(
			"Q :: use foo.P\nsum :: fn(q: Q) int { q.x + q.y }\nprint(sum(Q.{ x = 3, y = 4 }))",
			"pub P :: struct { x: int, y: int }",
		),
		(
			"use foo.{ E }\ne := E.a\nprint(match e { E.a => 7, E.b => 0 })",
			"pub E :: enum { a, b }",
		),
		("use foo.{ Id }\nn: Id = 7\nprint(n)", "pub Id :: int"),
		("use foo.{ id }\nn: id = 7\nprint(n)", "pub id :: int"),
	] {
		let p = Project::new()
			.file("main.oi", ["module main", main])
			.file("foo/lib.oi", ["module foo", lib]);
		assert_eq!(ok(run_main(p)), "7");
	}
}

#[test]
fn type_import_fails() {
	for (lib, expected) in [
		("P :: struct { x: int }", "private to module `foo`"),
		("pub P :: trait {}", "cannot be imported"),
	] {
		let p = Project::new()
			.file("main.oi", ["module main", "use foo.{ P }", "print(1)"])
			.file("foo/lib.oi", ["module foo", lib]);
		let out = err(run_main(p));
		assert!(out.contains(expected), "{out}");
	}
}

#[test]
fn type_reexport() {
	let p = Project::new()
		.file("main.oi", ["module main", "use mid.{ P }", "print(P.{ x = 7 }.x)"])
		.file("mid/lib.oi", ["module mid", "pub use base.P"])
		.file("base/lib.oi", ["module base", "pub P :: struct { x: int }"]);
	assert_eq!(ok(run_main(p)), "7");
}

#[test]
fn narrowed_import() {
	for main in [
		"io :: use foo.{ hi }\nprint(io.hi())",
		"io :: use foo.{ h :: hi }\nprint(io.h())",
	] {
		let p = Project::new()
			.file("main.oi", ["module main", main])
			.file("foo/lib.oi", ["module foo", "pub hi :: fn() int { 7 }"]);
		assert_eq!(ok(run_main(p)), "7");
	}
}

#[test]
fn narrowed_import_fails() {
	for (main, expected) in [
		("io :: use foo.{ hi }\nprint(io.yo())", "not part of"),
		("io :: use foo.{ nope }\nprint(io.nope())", "has no `nope`"),
	] {
		let p = Project::new().file("main.oi", ["module main", main]).file(
			"foo/lib.oi",
			["module foo", "pub hi :: fn() int { 7 }", "pub yo :: fn() int { 8 }"],
		);
		let out = err(run_main(p));
		assert!(out.contains(expected), "{out}");
	}
}

#[test]
fn reexport() {
	for (mid, call) in [
		("pub use base.hi", "mid.hi()"),
		("pub use base.{ hi }", "mid.hi()"),
		("pub use base.{ h :: hi }", "mid.h()"),
	] {
		let p = Project::new()
			.file("main.oi", ["module main", "use mid", &format!("print({call})")])
			.file("mid/lib.oi", ["module mid", mid])
			.file("base/lib.oi", ["module base", "pub hi :: fn() int { 7 }"]);
		assert_eq!(ok(run_main(p)), "7");
	}
}

#[test]
fn reexport_selected() {
	let p = Project::new()
		.file("main.oi", ["module main", "use mid.{ hi }", "print(hi())"])
		.file("mid/lib.oi", ["module mid", "pub use base.hi"])
		.file("base/lib.oi", ["module base", "pub hi :: fn() int { 7 }"]);
	assert_eq!(ok(run_main(p)), "7");
}

#[test]
fn reexport_chain() {
	let p = Project::new()
		.file("main.oi", ["module main", "use top", "print(top.hi())"])
		.file("top/lib.oi", ["module top", "pub use mid.hi"])
		.file("mid/lib.oi", ["module mid", "pub use base.hi"])
		.file("base/lib.oi", ["module base", "pub hi :: fn() int { 7 }"]);
	assert_eq!(ok(run_main(p)), "7");
}

#[test]
fn reexport_fails() {
	for (mid, expected) in [
		("use base.hi", "has no function `hi`"),
		("pub use base", "only item imports can be re-exported"),
		("pub io :: use base.{ hi }", "only item imports can be re-exported"),
	] {
		let p = Project::new()
			.file("main.oi", ["module main", "use mid", "print(mid.hi())"])
			.file("mid/lib.oi", ["module mid", mid])
			.file("base/lib.oi", ["module base", "pub hi :: fn() int { 7 }"]);
		let out = err(run_main(p));
		assert!(out.contains(expected), "{out}");
	}
}

#[test]
fn const_import() {
	for main in [
		"use foo\nprint(foo.name)",
		"use foo.{ name }\nprint(name)",
		"use foo.name\nprint(name)",
	] {
		let p = Project::new()
			.file("main.oi", ["module main", main])
			.file("foo/lib.oi", ["module foo", "pub name :: 7"]);
		assert_eq!(ok(run_main(p)), "7");
	}
}

#[test]
fn const_import_fails() {
	for (lib, expected) in [
		("name :: 7", "private to module `foo`"),
		("pub name := 7", "must be a const"),
		("pub name :: 1 + 2", "only literal consts"),
	] {
		let p = Project::new()
			.file("main.oi", ["module main", "use foo.{ name }", "print(name)"])
			.file("foo/lib.oi", ["module foo", lib]);
		let out = err(run_main(p));
		assert!(out.contains(expected), "{out}");
	}
}

#[test]
fn const_reexport() {
	let p = Project::new()
		.file("main.oi", ["module main", "use mid", "print(mid.name)"])
		.file("mid/lib.oi", ["module mid", "pub use base.name"])
		.file("base/lib.oi", ["module base", "pub name :: 7"]);
	assert_eq!(ok(run_main(p)), "7");
}

#[test]
fn module_cannot_call_main_private_fn() {
	let p = Project::new()
		.file(
			"main.oi",
			["module main", "secret :: fn() int { 1 }", "use foo", "print(foo.hi())"],
		)
		.file("foo/lib.oi", ["module foo", "pub hi :: fn() int { secret() }"]);
	let out = err(run_main(p));
	assert!(out.contains("undefined function"), "{out}");
}

#[test]
fn module_fn_uses_builtins_and_prints() {
	let p = Project::new().file("main.oi", ["module main", "use foo", "foo.go()"]).file(
		"foo/lib.oi",
		["module foo", "pub go :: fn() { n: int = 3\nprint(n + 1) }"],
	);
	assert_eq!(ok(run_main(p)), "4");
}

#[test]
fn exec_resolves_imports_against_cwd() {
	let p = Project::new().file("foo/lib.oi", ["module foo", "pub hi :: fn() int { 99 }"]);
	let out = ok(oi(&["exec", "use foo\nprint(foo.hi())"]).current_dir(&p).run(None));
	assert_eq!(out, "99");
}
