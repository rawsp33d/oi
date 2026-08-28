use std::path::Path;

use crate::Reported;
use crate::compiler::Compiler;
use crate::loader;

/// Compile and run a program from its source text.
///
/// `name` labels the source in diagnostics (a file path, or `<exec>` / `<stdin>`).
/// `root` anchors module lookups.
/// On failure the diagnostic is rendered to stderr.
pub fn run_source(name: &str, src: &str, root: &Path) -> Result<(), Reported> {
	let program = loader::load(name, src.to_string(), root)?;

	let mut compiler = Compiler::default();
	let code = match compiler.compile(&program) {
		Ok(code) => code,
		Err(error) => {
			error.report_mapped(&program.map);
			return Err(Reported);
		}
	};

	// run
	// SAFETY: `code` is the finalized `__oi_main` entrypoint emitted by `compile`. There are no params or return.
	let f = unsafe { std::mem::transmute::<*const u8, fn()>(code) };
	f();
	crate::runtime::collect_cycles();
	if std::env::var_os("OI_LEAK_CHECK").is_some() {
		eprintln!("leaked allocations: {}", crate::runtime::leaked());
	}
	Ok(())
}

/// Compile a program in test mode and run every `@test` fn in the main module.
pub fn test_source(name: &str, src: &str, root: &Path, pattern: Option<&str>) -> Result<(), Reported> {
	let program = loader::load(name, src.to_string(), root)?;
	let mut compiler = Compiler::default();
	compiler.include_tests = true;
	if let Err(error) = compiler.compile(&program) {
		error.report_mapped(&program.map);
		return Err(Reported);
	}
	let total = compiler.tests.len();
	if let Some(pattern) = pattern {
		compiler.tests.retain(|(_, display, _)| display.contains(pattern));
	}
	let filtered = total - compiler.tests.len();
	let mut passed = 0;
	let mut skipped = 0;
	for (fn_name, display, skip) in &compiler.tests {
		print!("test {display} ... ");
		std::io::Write::flush(&mut std::io::stdout()).ok();
		if *skip {
			println!("skipped");
			skipped += 1;
			continue;
		}
		// SAFETY: there are no other threads, the child only runs the fn and exits
		let ok = match unsafe { libc::fork() } {
			0 => {
				compiler.finalized_test(fn_name)();
				std::process::exit(0)
			}
			-1 => {
				eprintln!("oi: fork failed");
				return Err(Reported);
			}
			pid => {
				let mut status = 0;
				// SAFETY: `pid` is our own child, `status` is a valid out-pointer
				unsafe { libc::waitpid(pid, &mut status, 0) };
				libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0
			}
		};
		println!("{}", if ok { "ok" } else { "FAILED" });
		passed += ok as usize;
	}
	let failed = compiler.tests.len() - passed - skipped;
	let tail: String = [(failed, "failed"), (skipped, "skipped"), (filtered, "filtered out")]
		.iter()
		.filter(|(n, _)| *n > 0)
		.map(|(n, what)| format!("; {n} {what}"))
		.collect();
	println!("{passed} passed{tail}");
	if failed == 0 { Ok(()) } else { Err(Reported) }
}
