use std::io::Read as _;

use oi::Reported;
use oi::driver::run_source;

/// Compile and run source passed as an argument, or read from stdin.
pub fn run(source: Option<String>) -> Result<(), Reported> {
	let (name, src) = match source {
		Some(src) => ("<exec>", src),
		None => {
			let mut src = String::new();
			std::io::stdin().read_to_string(&mut src).map_err(|e| {
				eprintln!("oi: cannot read stdin: {e}");
				Reported
			})?;
			("<stdin>", src)
		}
	};
	run_source(name, &src, false)
}
