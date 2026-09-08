use crate::common::{Project, Run, oi, ok};

#[test]
fn scaffolds_a_runnable_project() {
	let dir = Project::new();
	ok(oi(&["new", "hello"]).current_dir(&dir).run(None));

	let hello = dir.as_ref().join("hello");
	assert!(hello.join(".gitignore").exists());
	assert!(hello.join(".git").exists());
	assert_eq!(ok(oi(&["run"]).current_dir(&hello).run(None)), "Hi Mom!");
}
