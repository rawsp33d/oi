use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// The Oi CLI.
#[derive(Parser)]
#[command(name = "oi", version, about)]
pub struct Cli {
	#[command(subcommand)]
	pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
	/// Compile and run an Oi file.
	Run {
		/// Path to the .oi source file.
		#[arg(default_value = "main.oi")]
		file: PathBuf,
	},

	/// Compile an Oi file to a native executable.
	Build {
		/// Path to the source file.
		#[arg(default_value = "main.oi")]
		file: PathBuf,

		/// Output path.
		/// Defaults to the file stem in the current directory.
		#[arg(short, long)]
		out: Option<PathBuf>,

		/// Build a shared library instead of an executable.
		#[arg(long)]
		lib: bool,
	},

	/// Compile and run an Oi script.
	Exec {
		/// Source to run, appended to piped stdin if any. If omitted, read from stdin.
		#[arg(allow_hyphen_values = true)]
		source: Option<String>,
	},

	/// Compile and run a file's `@test` fns.
	Test {
		/// Path to the .oi source file.
		#[arg(default_value = "main.oi")]
		file: PathBuf,

		/// Only run tests whose name matches pattern.
		pattern: Option<String>,
	},

	/// Start an interactive Oi REPL.
	Repl,
}
