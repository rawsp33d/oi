mod cli;
mod commands;

use std::process::ExitCode;

use clap::Parser as _;

use crate::cli::Cli;

fn main() -> ExitCode {
	match commands::dispatch(Cli::parse().command.unwrap_or(cli::Command::Repl)) {
		Ok(()) => ExitCode::SUCCESS,
		Err(_) => ExitCode::FAILURE,
	}
}
