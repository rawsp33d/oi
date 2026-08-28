use crate::common::{Project, Run, oi, ok};

#[test]
fn missing_file_errors() {
	let out = oi(&["run", "definitely-missing.oi"]).run(None);
	assert!(!out.status.success());
	let stderr = String::from_utf8_lossy(&out.stderr);
	assert!(stderr.contains("cannot read"), "stderr was:\n{stderr}");
}

#[test]
fn default_file_is_main_oi_in_cwd() {
	let dir = Project::new().file("main.oi", "1 + 2");
	assert_eq!(ok(oi(&["run"]).current_dir(&dir).run(None)), "3");
}
