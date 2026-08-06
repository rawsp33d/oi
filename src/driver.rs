use std::path::Path;

use crate::Reported;
use crate::compiler::Compiler;
use crate::loader;

/// Compile and run a program from its source text.
///
/// `name` labels the source in diagnostics (a file path, or `<exec>` / `<stdin>`).
/// `root` anchors module lookups.
/// On failure the diagnostic is rendered to stderr.
pub fn run_source(name: &str, src: &str, root: &Path, debug_ast: bool) -> Result<(), Reported> {
	let program = loader::load(name, src.to_string(), root)?;

	if debug_ast {
		for m in &program.modules {
			eprintln!("{:#?}", m.items);
		}
	}

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
