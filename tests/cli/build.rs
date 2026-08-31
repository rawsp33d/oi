use std::process::Command;

use indoc::indoc;

use crate::common::{Project, Run, oi, ok, trim};

/// Build main.oi with, and run the binary.
fn build_and_run(src: &str, args: &[&str], bin: &str) -> String {
	let dir = Project::new().file("main.oi", src);
	let out = oi(&[&["build"][..], args].concat()).current_dir(&dir).run(None);
	assert!(
		out.status.success(),
		"build failed:\n{}",
		String::from_utf8_lossy(&out.stderr)
	);
	let run = std::process::Command::new(dir.as_ref().join(bin)).output().unwrap();
	String::from_utf8_lossy(&run.stdout).trim().to_string()
}

#[test]
fn hello_world() {
	assert_eq!(build_and_run(r#"print("hi")"#, &["main.oi", "-o", "hi"], "hi"), "hi");
}

#[test]
fn macros_comp_foreign_and_leak_check() {
	let main = indoc! {r#"
		use util
		sq! :: fn(e: Ast) Ast { `%e * %e` }
		print(sq!(comp 2 + 3))
		print(util.cube(3))
	"#};
	let util = indoc! {"
		module util
		pub cube :: fn(n: int) int { oi_pow_int(n, 3) }
		oi_pow_int : fn(base: int, exp: int) int : foreign
	"};
	let dir = Project::new().file("main.oi", main).file("util.oi", util);
	ok(oi(&["build"]).current_dir(&dir).run(None));
	let mut bin = Command::new(dir.as_ref().join("main"));
	let out = bin.env("OI_LEAK_CHECK", "1").output().unwrap();
	assert_eq!(trim(&out.stdout), "25\n27");
	assert_eq!(trim(&out.stderr), "leaked allocations: 0");
}

#[test]
fn structs_traits_and_generics_reach_the_linker() {
	let src = indoc! {r#"
		Animal :: trait { speak : fn(self) string }
		Dog :: struct { name: string }
		Dog : Animal < { speak :: fn(self) string { self.name + " woofs" } }
		Opt[T] :: enum { nope, some(T) }
		wrap[T] :: fn(v: T) Opt[T] { .some(v) }
		match wrap(Dog.{ "Rex" }) {
			.some(d) => print(d.speak()),
			.nope => print("none"),
		}
	"#};
	assert_eq!(build_and_run(src, &[], "main"), "Rex woofs");
}
