use std::process::{Command, Stdio};

use crate::common::{Project, Run, oi, ok};

#[test]
fn installs_module_into_home() {
	let home = Project::new();
	let p = Project::new().file("greet/lib.oi", ["module greet", "pub hi :: fn() int { 42 }"]);
	let install = |args: &[&str]| ok(oi(args).current_dir(&p).env("OI_HOME", home.as_ref()).run(None));
	install(&["install", "greet", "--link"]);
	assert!(home.as_ref().join("lib/greet").is_symlink());
	install(&["install", "greet"]);
	assert!(!home.as_ref().join("lib/greet").is_symlink());
	assert!(p.as_ref().join("greet/lib.oi").metadata().unwrap().len() > 0);

	let main = Project::new().file("main.oi", ["module main", "use greet", "print(greet.hi())"]);
	let out = ok(oi(&["run", "main.oi"])
		.current_dir(&main)
		.env("OI_HOME", home.as_ref())
		.run(None));
	assert_eq!(out, "42");
}

#[test]
fn installs_binary_into_home() {
	let home = Project::new();
	let p = Project::new().file("src/main.oi", ["main :: fn() { print(\"hi\") }"]);
	ok(oi(&["install"]).current_dir(&p).env("OI_HOME", home.as_ref()).run(None));
	let bin = home.as_ref().join("bin").join(p.as_ref().file_name().unwrap());
	assert_eq!(ok(Command::new(bin).stdout(Stdio::piped()).run(None)), "hi");
}
