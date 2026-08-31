pub mod exec;
pub mod repl;
pub mod run;

use oi::Reported;

use crate::cli::Command;

/// Route a parsed command to its handler.
pub fn dispatch(command: Command) -> Result<(), Reported> {
	match command {
		Command::Run { file } => run::run(&file),
		Command::Build { file, out } => run::build(&file, out.as_deref()),
		Command::Exec { source } => exec::run(source),
		Command::Test { file, pattern } => run::test(&file, pattern.as_deref()),
		Command::Repl => repl::run(),
	}
}
