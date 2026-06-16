mod ast;
mod compiler;
mod lexer;
mod parser;
mod runtime;

use crate::lexer::*;
use crate::parser::*;

use chumsky::{input::Stream, prelude::*};
use logos::{Logos, Span};

fn lex(src: &str) -> Vec<(Token, Span)> {
	let lexer = Token::lexer(src);
	let mut tokens = vec![];
	for (token, span) in lexer.spanned() {
		match token {
			Ok(t) => tokens.push((t, span)),
			Err(()) => panic!("{:?}", span),
		}
	}
	tokens
}

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
	let src = std::fs::read_to_string(file).unwrap();

	let lexed = lex(&src);
	let stream = Stream::from_iter(lexed.into_iter().map(|(t, s)| (t, s.into())))
		.map((src.len()..src.len()).into(), |(t, s)| (t, s));
	let ast = parser().parse(stream).into_result().unwrap();

	if debug_ast {
		eprintln!("{ast:#?}");
	}

	let mut compiler = compiler::Compiler::default();
	let code = compiler.compile(&ast).unwrap();
	let f = unsafe { std::mem::transmute::<*const u8, fn()>(code) };
	f();
}
