mod ast;
mod cli;
mod compiler;
mod diagnostics;
mod lexer;
mod parser;
mod runtime;

use std::io::Read;
use std::path::Path;

use crate::cli::{Cli, Command};
use crate::diagnostics::Diagnostic;
use crate::lexer::lex;
use crate::parser::parser;

use chumsky::{input::Stream, prelude::*};
use clap::Parser as _;

fn main() {
	let cli = Cli::parse();
	match cli.command {
		Command::Run { file, debug_ast } => run(&file, debug_ast),
		Command::Exec { source, debug_ast } => exec(source, debug_ast),
	}
}

/// Compile and run a file.
fn run(file: &Path, debug_ast: bool) {
	let src = std::fs::read_to_string(file).unwrap_or_else(|e| {
		eprintln!("oi: cannot read {}: {e}", file.display());
		std::process::exit(1);
	});
	run_source(&file.display().to_string(), &src, debug_ast);
}

/// Compile and run a script passed as an argument or read from stdin.
fn exec(source: Option<String>, debug_ast: bool) {
	let (name, src) = match source {
		Some(src) => ("<exec>", src),
		None => {
			let mut src = String::new();
			std::io::stdin()
				.read_to_string(&mut src)
				.unwrap_or_else(|e| {
					eprintln!("oi: cannot read stdin: {e}");
					std::process::exit(1);
				});
			("<stdin>", src)
		}
	};
	run_source(name, &src, debug_ast);
}

/// Compile and run a program.
fn run_source(name: &str, src: &str, debug_ast: bool) {
	// lex
	let tokens = lex(src);
	let stream = Stream::from_iter(tokens).map((src.len()..src.len()).into(), |(t, s)| (t, s));
	// parse
	let ast = parser()
		.parse(stream)
		.into_result()
		.unwrap_or_else(|errors| {
			for e in &errors {
				Diagnostic::from_rich(e).report(name, src);
			}
			std::process::exit(1);
		});

	if debug_ast {
		eprintln!("{ast:#?}");
	}

	// compile
	let mut compiler = compiler::Compiler::default();
	let code = compiler.compile(&ast).unwrap_or_else(|error| {
		// report errors
		error.report(name, src);
		std::process::exit(1);
	});

	// run
	let f = unsafe { std::mem::transmute::<*const u8, fn()>(code) };
	f();
}
