use std::path::Path;
use std::process::Command;

use oi::Reported;

/// Scaffold a project in cwd, or `name` if provided.
pub fn run(name: Option<&str>) -> Result<(), Reported> {
	let dir = Path::new(name.unwrap_or("."));
	let entry = dir.join("src/main.oi");
	if entry.exists() {
		eprintln!("oi: {} already exists", entry.display());
		return Err(Reported);
	}

	write(&entry, r#"main :: fn() {\n\twho :: "Mom"\n\tprint("hi {who}!")\n}\n"#)?;
	let ignore = dir.join(".gitignore");
	if !ignore.exists() {
		write(&ignore, "# oi build output\n/main\n/lib*\n")?;
	}
	if !dir.canonicalize().is_ok_and(|p| p.ancestors().any(|a| a.join(".git").exists())) {
		Command::new("git").args(["init", "--quiet"]).arg(dir).status().ok();
	}

	println!("oi: created {}", entry.display());
	Ok(())
}

/// Write a file, creating parent directories.
fn write(path: &Path, content: &str) -> Result<(), Reported> {
	std::fs::create_dir_all(path.parent().unwrap_or(path))
		.and_then(|()| std::fs::write(path, content))
		.map_err(|e| {
			eprintln!("oi: cannot write {}: {e}", path.display());
			Reported
		})
}
