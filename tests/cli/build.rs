use indoc::indoc;

use crate::common::{Project, Run, oi};

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
