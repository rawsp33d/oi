pub mod exec;
pub mod init;
pub mod repl;
pub mod run;

use oi::Reported;

use crate::cli::Command;

/// Route a parsed command to its handler.
pub fn dispatch(cmd: Command) -> Result<(), Reported> {
	match cmd {
		Command::Init { name } => init::run(name.as_deref()),
		Command::Run { file } => run::run(&run::entry(file)),
		Command::Build { file, out, lib } => run::build(&run::entry(file), out.as_deref(), lib),
		Command::Exec { source } => exec::run(source),
		Command::Test { file, pattern } => run::test(&run::entry(file), pattern.as_deref()),
		Command::Repl => repl::run(),
	}
}
