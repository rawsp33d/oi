use indoc::indoc;

use crate::common::{Project, Run, oi, ok};

fn project(src: &str) -> Project {
	Project::new().file("main.oi", src)
}

#[test]
fn run_strips_tests_before_typecheck() {
	let dir = project(indoc! {r#"
		@test bad :: fn() { 1 < "x" }
		main :: fn() { print("fine") }
	"#});
	assert_eq!(ok(oi(&["run"]).current_dir(&dir).run(None)), "fine");
}

#[test]
fn test_runs_all_in_order() {
	let dir = project(indoc! {r#"
		@test first :: fn() { assert!(1 + 1 == 2) }
		@test second :: fn() { assert!(2 + 2 == 4) }
	"#});
	let out = ok(oi(&["test"]).current_dir(&dir).run(None));
	assert!(out.find("first").unwrap() < out.find("second").unwrap() && out.contains("2 passed"));
}

#[test]
fn failing_test_exits_nonzero() {
	let dir = project(indoc! {r#"
		@test first :: fn() { assert!(true) }
		@test second :: fn() { assert!(false) }
	"#});
	let out = oi(&["test"]).current_dir(&dir).run(None);
	assert!(!out.status.success() && String::from_utf8_lossy(&out.stdout).contains("first"));
}
