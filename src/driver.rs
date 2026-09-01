use std::io::Write as _;
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
	crate::runtime::epilogue();
	Ok(())
}

/// The static runtime, embedded so the compiler is a single self-contained binary.
static RUNTIME: &[u8] = include_bytes!(env!("CARGO_STATICLIB_FILE_OI_RUNTIME_oi_runtime"));

// What the runtime staticlib needs linked in.
// `rustc --print native-static-libs`.
#[cfg(target_os = "linux")]
const LIBS: &[&str] = &["-lgcc_s", "-lutil", "-lrt", "-lpthread", "-lm", "-ldl"];
#[cfg(target_os = "macos")]
const LIBS: &[&str] = &["-lSystem", "-lc", "-lm"];
#[cfg(windows)]
const LIBS: &[&str] = &[
	"-lkernel32",
	"-ladvapi32",
	"-lbcrypt",
	"-lntdll",
	"-luserenv",
	"-lws2_32",
];

/// Compile a program to a native executable at `out`, linked against the static runtime.
/// With `lib`, emits a shared library exporting `oi_init` plus every `pub` free fn instead.
pub fn build_source(name: &str, src: &str, root: &Path, out: &Path, lib: bool) -> Result<(), Reported> {
	let program = loader::load(name, src.to_string(), root)?;
	let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or("main");
	let (obj, link_libs) = Compiler::object(stem, lib).compile_object(&program).map_err(|e| {
		e.report_mapped(&program.map);
		Reported
	})?;
	let tmp = std::env::temp_dir().join(format!("oi_{}", std::process::id()));
	let write = |ext: &str, bytes: &[u8]| {
		let path = tmp.with_extension(ext);
		std::fs::write(&path, bytes)
			.map(|_| path)
			.map_err(|e| fail(format!("cannot write {}: {e}", tmp.display())))
	};
	let cc = std::process::Command::new("cc")
		.args([write("o", &obj)?, write("a", RUNTIME)?])
		.args(if lib { &["-shared"][..] } else { &[] })
		.args(LIBS)
		.args(link_libs.iter().map(|l| format!("-l{l}")))
		.arg("-o")
		.arg(out)
		.output();
	for ext in ["o", "a"] {
		std::fs::remove_file(tmp.with_extension(ext)).ok();
	}
	let cc = cc.map_err(|e| fail(format!("cc: {e}")))?;
	if !cc.status.success() {
		std::io::stderr().write_all(&cc.stderr).ok();
		return Err(Reported);
	}
	Ok(())
}

fn fail(msg: impl std::fmt::Display) -> Reported {
	eprintln!("oi: {msg}");
	Reported
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
