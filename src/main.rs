mod ast;
mod compiler;
mod diagnostics;
mod lexer;
mod parser;
mod runtime;

use crate::diagnostics::Diagnostic;
use crate::lexer::lex;
use crate::parser::parser;

use chumsky::{input::Stream, prelude::*};

fn main() {
	let mut file = None;
	let mut debug_ast = false;

	for arg in std::env::args().skip(1) {
		match arg.as_str() {
			"--debug-ast" => debug_ast = true,
			_ => file = Some(arg),
		}
	}

	let file = file.unwrap_or_else(|| "examples/main.oi".into());
	let src = std::fs::read_to_string(&file).unwrap_or_else(|e| {
		eprintln!("oi: cannot read {file}: {e}");
		std::process::exit(1);
	});

	// lex
	let tokens = lex(&src);
	let stream = Stream::from_iter(tokens).map((src.len()..src.len()).into(), |(t, s)| (t, s));
	// parse
	let ast = parser()
		.parse(stream)
		.into_result()
		.unwrap_or_else(|errors| {
			for e in &errors {
				Diagnostic::from_rich(e).report(&file, &src);
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
		error.report(&file, &src);
		std::process::exit(1);
	});

	// run
	let f = unsafe { std::mem::transmute::<*const u8, fn()>(code) };
	f();
}
