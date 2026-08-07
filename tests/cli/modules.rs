use std::process::Output;

use crate::common::{Project, Run, err, oi, ok};

// Run main.oi in a project.
fn run_main(p: Project) -> Output {
	oi(&["run", "main.oi"]).current_dir(&p).run(None)
}

#[test]
fn imports_work() {
	let p = Project::new()
		.file("main.oi", ["module main", "import foo", "print(foo.total())"])
		.file(
			"foo/lib.oi",
			[
				"module foo",
				"import bar",
				"struct P { x int, y int }",
				"fn sum(p P) int { p.x + p.y }",
				"pub fn total() int { bar.twice(sum(P{x: 2, y: 5})) }",
			],
		)
		.file("bar/lib.oi", ["module bar", "pub fn twice(n int) int { n * 2 }"]);
	assert_eq!(ok(run_main(p)), "14");
}

#[test]
fn private_fn_rejected() {
	let p = Project::new()
		.file("main.oi", ["module main", "import foo", "print(foo.secret())"])
		.file("foo/lib.oi", ["module foo", "fn secret() int { 1 }"]);
	let out = err(run_main(p));
	assert!(out.contains("private to module `foo`"), "{out}");
}

#[test]
fn wrong_module_decl_names_the_file() {
	let p = Project::new()
		.file("main.oi", ["module main", "import foo", "print(foo.hi())"])
		.file("foo/lib.oi", ["module bar", "pub fn hi() int { 1 }"]);
	let out = err(run_main(p));
	assert!(out.contains("foo/lib.oi"), "{out}");
	assert!(out.contains("module foo"), "{out}");
}

#[test]
fn import_cycle_rejected() {
	let p = Project::new()
		.file("main.oi", ["module main", "import a", "print(a.v())"])
		.file("a/m.oi", ["module a", "import b", "pub fn v() int { b.v() }"])
		.file("b/m.oi", ["module b", "import a", "pub fn v() int { a.v() }"]);
	let out = err(run_main(p));
	assert!(out.contains("import cycle"), "{out}");
}

#[test]
fn duplicate_name_across_files_rejected() {
	let p = Project::new()
		.file("main.oi", ["module main", "import foo", "print(foo.hi())"])
		.file("foo/a.oi", ["module foo", "pub fn hi() int { 1 }"])
		.file("foo/b.oi", ["module foo", "fn hi() int { 2 }"]);
	let out = err(run_main(p));
	assert!(out.contains("defined twice"), "{out}");
}

#[test]
fn exec_resolves_imports_against_cwd() {
	let p = Project::new().file("foo/lib.oi", ["module foo", "pub fn hi() int { 99 }"]);
	let out = ok(oi(&["exec", "import foo\nprint(foo.hi())"]).current_dir(&p).run(None));
	assert_eq!(out, "99");
}
