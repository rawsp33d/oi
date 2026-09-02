pub mod exec;
pub mod repl;
pub mod run;

use oi::Reported;

use crate::cli::Command;

/// Route a parsed command to its handler.
pub fn dispatch(cmd: Command) -> Result<(), Reported> {
	match cmd {
		Command::Run { file } => run::run(&file),
		Command::Build { file, out, lib } => run::build(&file, out.as_deref(), lib),
		Command::Exec { source } => exec::run(source),
		Command::Test { file, pattern } => run::test(&file, pattern.as_deref()),
		Command::Repl => repl::run(),
	}
}
