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

#[test]
fn timings_prints_phases_to_stderr() {
	let dir = Project::new().file("main.oi", "1 + 2");
	let out = oi(&["run", "--timings"]).current_dir(&dir).run(None);
	assert!(out.status.success());
	let stderr = String::from_utf8_lossy(&out.stderr);
	assert!(stderr.contains("codegen"), "stderr was:\n{stderr}");
	assert!(stderr.contains("run"), "stderr was:\n{stderr}");
}
