use crate::ast::{BinOp, Capture, EnumVariant, Expr, MatchArm, Param, Pattern, Span, Spanned, TypeExpr, TypeParam};
use crate::lexer::Token;

use chumsky::{
	input::ValueInput,
	pratt::{infix, left, postfix, prefix},
	prelude::*,
};

// The contents of a subscript.
enum Subscript {
	Index(Spanned<Expr>),
	Slice(Option<Spanned<Expr>>, Option<Spanned<Expr>>),
}

// field/tuple/method access
enum Access {
	Fields(Vec<String>),
	Method(String, Vec<Spanned<Expr>>),
}

fn pipe_step((e, span): Spanned<Expr>) -> Spanned<Expr> {
	match e {
		Expr::Ident(name) => (
			Expr::Call {
				name,
				type_args: vec![],
				args: vec![(Expr::Dollar, span)],
			},
			span,
		),
		Expr::Propagate(inner) => (Expr::Propagate(Box::new(pipe_step(*inner))), span),
		e => (e, span),
	}
}

// `value |> step`
fn pipe(value: Spanned<Expr>, step: Spanned<Expr>, span: Span) -> Spanned<Expr> {
	(
		Expr::Pipe {
			value: Box::new(value),
			step: Box::new(pipe_step(step)),
		},
		span,
	)
}

// Prepend $ to LHS of pipeline.
fn dollar_pipe((e, span): Spanned<Expr>) -> Spanned<Expr> {
	match e {
		Expr::Pipe { value, step } => (
			Expr::Pipe {
				value: Box::new(dollar_pipe(*value)),
				step,
			},
			span,
		),
		e => pipe((Expr::Dollar, span), (e, span), span),
	}
}

// Assemble a fn item.
fn fn_def(
	(name, mut type_params): (String, Vec<TypeParam>),
	params: Option<(Vec<Param>, bool)>,
	ret: Option<Spanned<TypeExpr>>,
	body: Vec<Spanned<Expr>>,
	span: Span,
) -> Spanned<Expr> {
	let (params, params_tuple) = params.unwrap_or_else(|| {
		type_params.push(TypeParam {
			name: "$I".into(),
			bound: None,
		});
		let param = Param {
			name: "$".into(),
			typ: TypeExpr::Name("$I".into()),
			span,
			default: None,
			mutable: false,
		};
		(vec![param], false)
	});
	(
		Expr::Fn {
			name,
			type_params,
			params,
			params_tuple,
			ret,
			body,
		},
		span,
	)
}

// Wrap a value and `or` body into an `OrElse`.
fn or_else((value, body): (Spanned<Expr>, Option<Vec<Spanned<Expr>>>), span: Span) -> Spanned<Expr> {
	match body {
		Some(body) => (
			Expr::OrElse {
				value: Box::new(value),
				body,
			},
			span,
		),
		None => value,
	}
}

pub fn parser<'token, I>() -> impl Parser<'token, I, Vec<Spanned<Expr>>, extra::Err<Rich<'token, Token>>>
where
	I: ValueInput<'token, Token = Token, Span = SimpleSpan>,
{
	// `expr` and the statement/block parsers are mutually recursive
	// `if` is an expression, but its branches are statement blocks.
	// Declare `expr` up front so the statement parsers can reference it before it is defined.
	let mut expr = Recursive::declare();
	let mut header_expr = Recursive::declare();

	let ident = || select! { Token::Ident(name) => name };

	fn paren<'token, I, O, P>(p: P) -> impl Parser<'token, I, O, extra::Err<Rich<'token, Token>>> + Clone
	where
		I: ValueInput<'token, Token = Token, Span = SimpleSpan>,
		P: Parser<'token, I, O, extra::Err<Rich<'token, Token>>> + Clone,
	{
		p.delimited_by(just(Token::LParen), just(Token::RParen))
	}
	fn brace<'token, I, O, P>(p: P) -> impl Parser<'token, I, O, extra::Err<Rich<'token, Token>>> + Clone
	where
		I: ValueInput<'token, Token = Token, Span = SimpleSpan>,
		P: Parser<'token, I, O, extra::Err<Rich<'token, Token>>> + Clone,
	{
		p.delimited_by(just(Token::LBrace), just(Token::RBrace))
	}
	fn bracket<'token, I, O, P>(p: P) -> impl Parser<'token, I, O, extra::Err<Rich<'token, Token>>> + Clone
	where
		I: ValueInput<'token, Token = Token, Span = SimpleSpan>,
		P: Parser<'token, I, O, extra::Err<Rich<'token, Token>>> + Clone,
	{
		p.delimited_by(just(Token::LBracket), just(Token::RBracket))
	}
	fn list<'token, I, O, P>(p: P) -> impl Parser<'token, I, Vec<O>, extra::Err<Rich<'token, Token>>> + Clone
	where
		I: ValueInput<'token, Token = Token, Span = SimpleSpan>,
		P: Parser<'token, I, O, extra::Err<Rich<'token, Token>>> + Clone,
	{
		p.separated_by(just(Token::Comma)).collect::<Vec<_>>()
	}
	fn loose_list<'token, I, O, P>(p: P) -> impl Parser<'token, I, Vec<O>, extra::Err<Rich<'token, Token>>> + Clone
	where
		I: ValueInput<'token, Token = Token, Span = SimpleSpan>,
		P: Parser<'token, I, O, extra::Err<Rich<'token, Token>>> + Clone,
	{
		p.separated_by(just(Token::Comma).or_not()).allow_trailing().collect::<Vec<_>>()
	}
	fn spanned<'token, I, O, P>(p: P) -> impl Parser<'token, I, Spanned<O>, extra::Err<Rich<'token, Token>>> + Clone
	where
		I: ValueInput<'token, Token = Token, Span = SimpleSpan>,
		P: Parser<'token, I, O, extra::Err<Rich<'token, Token>>> + Clone,
	{
		p.map_with(|o, ex| (o, ex.span()))
	}

	// type annotations
	let type_expr = recursive(|te| {
		let base = recursive(|base| {
			let name = ident().map(TypeExpr::Name);
			let unit = just(Token::LParen).then(just(Token::RParen)).to(TypeExpr::Tuple(vec![]));
			let tuple = paren(
				te.clone()
					.separated_by(just(Token::Comma).or_not())
					.allow_trailing()
					.at_least(1)
					.collect::<Vec<_>>(),
			)
			.map(TypeExpr::Tuple);
			// arrays
			let array = just(Token::LBracket)
				.ignore_then(select! { Token::Int(n) => n }.or_not())
				.then_ignore(just(Token::RBracket))
				.then(base.clone())
				.map(|(n, elem)| match n {
					Some(n) => TypeExpr::FixedArray(Box::new(elem), n as usize),
					None => TypeExpr::Array(Box::new(elem)),
				});
			let fn_param = just(Token::Mut).or_not().then(te.clone());
			let fn_type =
				just(Token::Fn)
					.ignore_then(paren(loose_list(fn_param)))
					.then(base.clone())
					.map(|(params, ret)| {
						let (muts, params) = params.into_iter().map(|(m, t)| (m.is_some(), t)).unzip();
						TypeExpr::Fn(params, muts, Box::new(ret))
					});
			// options
			let option = just(Token::Question)
				.ignore_then(base.clone())
				.map(|t| TypeExpr::Option(Box::new(t)));
			// results
			let result = just(Token::Not)
				.ignore_then(base.clone())
				.map(|t| TypeExpr::Result(Box::new(t), None));
			// atom(s)
			let atom = select! { Token::Atom(a) => TypeExpr::AtomSum(vec![a]) };

			// built-in generic types
			let map_type = just(Token::Ident("Map".to_string()))
				.ignore_then(bracket(te.clone().then_ignore(just(Token::Comma)).then(te.clone())))
				.map(|(k, v)| TypeExpr::Map(Box::new(k), Box::new(v)));
			let result_long = just(Token::Ident("Result".to_string()))
				.ignore_then(bracket(te.clone().then_ignore(just(Token::Comma)).then(te.clone())))
				.map(|(t, e)| TypeExpr::Result(Box::new(t), Some(Box::new(e))));
			let option_long = just(Token::Ident("Option".to_string()))
				.ignore_then(bracket(te.clone()))
				.map(|t| TypeExpr::Option(Box::new(t)));
			// generic struct instantiation
			let generic_instance = ident()
				.then(bracket(
					te.clone().separated_by(just(Token::Comma)).at_least(1).collect::<Vec<_>>(),
				))
				.map(|(name, args)| TypeExpr::Generic(name, args));

			choice((
				unit,
				fn_type,
				option,
				result,
				atom,
				map_type,
				result_long,
				option_long,
				generic_instance,
				name,
				tuple,
				array,
			))
		});

		base.separated_by(just(Token::Pipe))
			.at_least(1)
			.collect::<Vec<_>>()
			.map(|mut ms| {
				if ms.len() == 1 {
					return ms.pop().unwrap();
				}
				let atom = |m: &TypeExpr| match m {
					TypeExpr::AtomSum(a) if a.len() == 1 => Some(a[0].clone()),
					_ => None,
				};
				match ms.iter().map(atom).collect::<Option<Vec<_>>>() {
					Some(names) => TypeExpr::AtomSum(names),
					None => TypeExpr::Sum(ms),
				}
			})
	})
	.boxed();

	// param type is kept for the compiler to resolve
	// NOTE: a bare `self` receiver gets the type `Self`
	let param = just(Token::Mut)
		.or_not()
		.then(ident())
		.then(type_expr.clone().or_not())
		.map_with(|((mutable, name), typ), ex| Param {
			typ: typ.unwrap_or(TypeExpr::Name("Self".into())),
			name,
			span: ex.span(),
			default: None,
			mutable: mutable.is_some(),
		});
	// NOTE: a trailing comma forces a tuple even for one param
	let params = paren(
		param
			.separated_by(just(Token::Comma))
			.collect::<Vec<_>>()
			.then(just(Token::Comma).or_not()),
	)
	.map(|(params, trailing)| {
		let tuple = params.len() != 1 || trailing.is_some();
		(params, tuple)
	})
	.boxed();

	// optional return type annotation
	let ret = spanned(type_expr.clone()).or_not();

	// generics
	let type_param = ident()
		.then(just(Token::Colon).ignore_then(ident()).or_not())
		.map(|(name, bound)| TypeParam { name, bound });
	let type_params = bracket(list(type_param)).or_not().map(Option::unwrap_or_default).boxed();

	// bindings
	let annot = spanned(type_expr.clone());
	let bind = just(Token::Mut)
		.or_not()
		.then(ident())
		.then(annot.clone().or_not())
		.then(just(Token::Bind).ignore_then(expr.clone()).or_not())
		.try_map(|(((mutable, name), typ), value), span| {
			if value.is_none() && (typ.is_none() || mutable.is_none()) {
				return Err(Rich::custom(span, "expected `:=` value, or `mut name type`"));
			}
			Ok(Expr::Bind {
				mutable: mutable.is_some(),
				name,
				typ,
				value: value.map(Box::new),
			})
		})
		.map_with(|e, ex| (e, ex.span()));

	// assignment
	let assign = ident()
		.then_ignore(just(Token::Assign))
		.then(expr.clone())
		.map_with(|(name, value), ex| {
			(
				Expr::Assign {
					name,
					value: Box::new(value),
				},
				ex.span(),
			)
		});

	// return statements
	let ret_stmt = just(Token::Return)
		.ignore_then(expr.clone().or_not())
		.map_with(|value, ex| (Expr::Return(value.map(Box::new)), ex.span()));

	// index assignment
	let index_assign = ident()
		.then(bracket(expr.clone()))
		.then_ignore(just(Token::Assign))
		.then(expr.clone())
		.map_with(|((name, index), value), ex| {
			(
				Expr::IndexAssign {
					name,
					index: Box::new(index),
					value: Box::new(value),
				},
				ex.span(),
			)
		});

	// array appending
	let append = ident()
		.then_ignore(just(Token::LtLt))
		.then(expr.clone())
		.map_with(|(name, value), ex| {
			(
				Expr::Append {
					name,
					value: Box::new(value),
				},
				ex.span(),
			)
		});

	// map deletion
	let map_delete = ident()
		.then_ignore(just(Token::Dot))
		.then_ignore(just(Token::Ident("delete".to_string())))
		.then(bracket(expr.clone()))
		.map_with(|(name, key), ex| {
			(
				Expr::MapDelete {
					name,
					key: Box::new(key),
				},
				ex.span(),
			)
		});

	// field assignment
	let field_assign = ident()
		.then_ignore(just(Token::Dot))
		.then(ident())
		.then_ignore(just(Token::Assign))
		.then(expr.clone())
		.map_with(|((name, field), value), ex| {
			(
				Expr::FieldAssign {
					name,
					field,
					value: Box::new(value),
				},
				ex.span(),
			)
		});

	// tuple destructuring
	let modded = list(just(Token::Mut).or_not().map(|m| m.is_some()).then(ident()));
	let plain = loose_list(ident().map(|n| (false, n)));
	let destructure = paren(modded)
		.or(paren(plain))
		.then(just(Token::Bind).to(true).or(just(Token::Assign).to(false)))
		.then(expr.clone())
		.try_map(|((names, bind), value), span| {
			if names.len() < 2 {
				return Err(Rich::custom(span, "tuple destructuring needs at least 2 names"));
			}
			if !bind && names.iter().any(|&(m, _)| m) {
				return Err(Rich::custom(span, "`mut` only applies to `:=` bindings"));
			}
			Ok(Expr::Destructure {
				names,
				bind,
				value: Box::new(value),
			})
		})
		.map_with(|e, ex| (e, ex.span()));

	let doc = select! { Token::Doc(text) => text }
		.repeated()
		.at_least(1)
		.collect::<Vec<_>>()
		.map_with(|lines, ex| (Expr::Doc(lines), ex.span()))
		.then_ignore(just(Token::DocBreak).or_not());

	// statements
	let stmt = doc
		.or(ret_stmt)
		.or(destructure)
		.or(bind)
		.or(field_assign)
		.or(assign)
		.or(index_assign)
		.or(map_delete)
		.or(append)
		.or(expr.clone())
		.boxed();

	// blocks
	let block = brace(stmt.clone().repeated().collect::<Vec<_>>()).boxed();

	let definition = {
		let literal = select! {
			Token::Bool(b) => Expr::Bool(b),
			Token::Int(n) => Expr::Int(n),
			Token::Float(s) => Expr::Float(s.parse().unwrap()),
			Token::String(s) => Expr::String(s),
			Token::Atom(name) => Expr::Atom(name),
			Token::Dollar => Expr::Dollar,
			Token::None => Expr::None,
		};

		// named args collect into one trailing record arg
		let named_arg = ident()
			.map_with(|n, ex| (Expr::Ident(n), ex.span()))
			.then_ignore(just(Token::Colon))
			.then(expr.clone())
			.map(|(key, value)| (Some(key), value));
		// arg mods
		let mut_arg = just(Token::Mut)
			.ignore_then(expr.clone())
			.map_with(|e, ex| (Expr::MutArg(Box::new(e)), ex.span()));
		// variable vs. call vs. struct literal
		let args = paren(
			named_arg
				.or(mut_arg.or(expr.clone()).map(|e| (None, e)))
				.separated_by(just(Token::Comma))
				.allow_trailing()
				.collect::<Vec<_>>(),
		)
		.validate(|elems, ex, emitter| {
			let mut args = Vec::new();
			let mut named = Vec::new();
			for (key, value) in elems {
				match key {
					Some(key) => named.push((key, value)),
					None if named.is_empty() => args.push(value),
					None => emitter.emit(Rich::custom(ex.span(), "positional args go before named args")),
				}
			}
			if !named.is_empty() {
				args.push((Expr::Record(named), ex.span()));
			}
			args
		})
		.boxed();

		// named or positional field entry
		let struct_field_entry = ident().then_ignore(just(Token::Colon)).or_not().then(expr.clone());
		let struct_body = brace(loose_list(struct_field_entry.clone()));

		// pull out struct literals separately (they have title case names) from vars/calls/whatever below
		let struct_lit = ident()
			.filter(|name| name.starts_with(char::is_uppercase))
			.then(struct_body)
			.map(|(name, fields)| Expr::StructLit { name, fields });

		// explicit generic types
		let call_type_args = bracket(
			spanned(type_expr.clone())
				.separated_by(just(Token::Comma))
				.at_least(1)
				.collect::<Vec<_>>(),
		);

		let var_or_call = ident()
			.then(call_type_args.or_not().then(args.clone()).or_not())
			.map(|(name, call)| match call {
				Some((type_args, args)) => Expr::Call {
					name,
					type_args: type_args.unwrap_or_default(),
					args,
				},
				None => Expr::Ident(name),
			});

		// leaf atoms pair themselves with their span
		let leaf = spanned(literal.or(struct_lit).or(var_or_call)).boxed();

		// record entries
		let key = select! {
			Token::Ident(name) => Expr::Ident(name),
			Token::Int(n) => Expr::Int(n),
			Token::String(s) => Expr::String(s),
			Token::Atom(a) => Expr::Atom(a),
		};
		let keyed = spanned(key).then(just(Token::Colon).ignore_then(expr.clone()));
		let pun = ident().map_with(|n, ex| ((Expr::Ident(n.clone()), ex.span()), (Expr::Ident(n), ex.span())));
		let record_entries = keyed
			.clone()
			.or(pun.then_ignore(just(Token::Comma).rewind()))
			.then_ignore(just(Token::Comma).or_not())
			.then(loose_list(keyed.clone().or(pun)))
			.map(|(first, mut rest)| {
				rest.insert(0, first);
				rest
			})
			.boxed();
		let record_arg = brace(record_entries.clone()).map_with(|es, ex| vec![(Expr::Record(es), ex.span())]);

		// enum shorthand
		let enum_shorthand = just(Token::Dot)
			.ignore_then(select! { Token::Ident(v) => v, Token::None => "none".to_string() })
			.then(args.clone().or(record_arg.clone()).or_not())
			.map_with(|(variant, args), ex| {
				let args = args.unwrap_or_default();
				(Expr::EnumShorthand { variant, args }, ex.span())
			})
			.boxed();

		// a lexer error token
		let bad = select! { Token::Error(text) => text }
			.try_map(|text, span| Err(Rich::custom(span, format!("unexpected character `{text}`"))));

		// grouping before tuple rule to avoid making 1ples, which are instead made with `(expr,)`
		let group = paren(expr.clone());

		// tuple literal
		let tuple = paren(loose_list(struct_field_entry)).map_with(|elems, ex| (Expr::Tuple(elems), ex.span()));

		// map literal
		let type_init = type_expr
			.clone()
			.filter(|t| matches!(t, TypeExpr::Array(_) | TypeExpr::FixedArray(..) | TypeExpr::Map(..)))
			.then(brace(loose_list(expr.clone())))
			.map_with(|(te, elems), ex| (Expr::TypeInit((te, ex.span()), elems), ex.span()));

		let option_init = just(Token::Question)
			.ignore_then(type_expr.clone())
			.then(paren(expr.clone()))
			.map_with(|(elem, arg), ex| {
				(
					Expr::OptionInit {
						inner: (elem, ex.span()),
						arg: Box::new(arg),
					},
					ex.span(),
				)
			});

		// result literal
		let result_shape = type_expr.clone().then(paren(expr.clone()));
		let result_init = just(Token::Not).ignore_then(result_shape.clone()).map_with(|(elem, arg), ex| {
			(
				Expr::ResultInit {
					inner: (elem, ex.span()),
					arg: Box::new(arg),
				},
				ex.span(),
			)
		});

		// array literal
		let array = bracket(loose_list(expr.clone())).map_with(|elems, ex| (Expr::Array(elems), ex.span()));

		// record literal
		let record = brace(record_entries.clone().or_not().map(Option::unwrap_or_default))
			.map_with(|entries, ex| (Expr::Record(entries), ex.span()));

		let if_expr = recursive(|if_expr| {
			just(Token::If)
				.ignore_then(header_expr.clone())
				.then(block.clone())
				.then(
					just(Token::Else)
						.ignore_then(if_expr.map(|e| vec![e]).or(block.clone()))
						.or_not(),
				)
				.map_with(|((cond, then), els), ex| {
					(
						Expr::If {
							cond: Box::new(cond),
							then,
							els,
						},
						ex.span(),
					)
				})
		})
		.boxed();

		// loops
		let loop_expr = just(Token::Loop)
			.ignore_then(
				block
					.clone()
					.map(|body| (None, body))
					.or(header_expr.clone().map(Some).then(block.clone())),
			)
			.map_with(|(cond, body), ex| {
				(
					Expr::Loop {
						cond: cond.map(Box::new),
						body,
					},
					ex.span(),
				)
			})
			.boxed();

		// a for-loop binds/destructures into names
		let pattern = {
			let name = ident();
			let tuple = paren(loose_list(name)).map(Pattern::Tuple);
			tuple.or(name.map(Pattern::Name))
		};
		let for_expr = just(Token::Loop)
			.ignore_then(pattern)
			.then_ignore(just(Token::In))
			.then(header_expr.clone().map(Box::new))
			.then(block.clone())
			.map_with(|((pat, iter), body), ex| (Expr::For { pat, iter, body }, ex.span()))
			.boxed();
		let break_expr = just(Token::Break).map_with(|_, ex| (Expr::Break, ex.span()));
		let continue_expr = just(Token::Continue).map_with(|_, ex| (Expr::Continue, ex.span()));

		// match expression
		let binding = ident().then_ignore(just(Token::At)).or_not();
		let arm_end = choice((
			just(Token::Comma).ignored(),
			just(Token::RBrace).rewind().ignored(),
			just(Token::Else).rewind().ignored(),
		));
		let arm_body = block
			.clone()
			.then_ignore(just(Token::Comma).or_not())
			.or(expr.clone().map(|e| vec![e]).then_ignore(arm_end));
		let struct_pat = just(Token::Dot)
			.ignore_then(select! { Token::Ident(v) => v })
			.then(brace(loose_list(keyed.clone().or(pun))))
			.map_with(|(variant, es), ex| {
				let args = vec![(Expr::Record(es), ex.span())];
				(Expr::EnumShorthand { variant, args }, ex.span())
			});
		let match_arm = binding
			.then(
				struct_pat
					.or(expr.clone())
					.separated_by(just(Token::Comma))
					.allow_trailing()
					.at_least(1)
					.collect::<Vec<_>>(),
			)
			.then_ignore(just(Token::FatArrow))
			.then(arm_body.clone())
			.map(|((binding, patterns), body)| MatchArm {
				binding,
				patterns,
				body,
			});
		let match_expr = just(Token::Match)
			.ignore_then(header_expr.clone())
			.then(brace(
				match_arm.repeated().collect::<Vec<_>>().then(
					just(Token::Else)
						.ignore_then(just(Token::FatArrow))
						.ignore_then(arm_body)
						.or_not(),
				),
			))
			.map_with(|(subject, (arms, else_body)), ex| {
				(
					Expr::Match {
						subject: Box::new(subject),
						arms,
						else_body,
					},
					ex.span(),
				)
			})
			.boxed();

		// anonymous functions
		let capture = ident();
		let capture = just(Token::Move)
			.ignore_then(capture)
			.map(Capture::Move)
			.or(just(Token::Mut).ignore_then(capture).map(Capture::Mut))
			.or(capture.map(Capture::ReadOnly));
		let captures = bracket(capture.separated_by(just(Token::Comma)).allow_trailing().collect::<Vec<_>>());
		let anon_fn = just(Token::Fn)
			.ignore_then(captures.or_not())
			.then(params.clone().or_not())
			.then(ret.clone())
			.then(block.clone())
			.map_with(|(((captures, params), ret), body), ex| {
				let (params, tuple) = params.unwrap_or((vec![], true));
				(
					Expr::AnonFn {
						captures,
						params,
						params_tuple: tuple,
						ret,
						body,
					},
					ex.span(),
				)
			})
			.boxed();

		// atoms
		let atom = choice((
			type_init,
			leaf,
			enum_shorthand,
			group,
			tuple,
			option_init,
			result_init,
			array,
			record,
			if_expr,
			match_expr,
			for_expr,
			loop_expr,
			break_expr,
			continue_expr,
			anon_fn.clone(),
			bad,
		))
		.boxed();

		// field/tuple/method access
		let access = choice((
			select! { Token::Int(n) => Access::Fields(vec![n.to_string()]) },
			// NOTE: chained tuple access like `x.0.1` lexes `0.1` as a float, hence the split
			select! { Token::Float(s) => Access::Fields(s.split('.').map(String::from).collect()) },
			ident()
				.then(args.clone().or(record_arg.clone()).or_not())
				.map(|(name, call)| match call {
					Some(args) => Access::Method(name, args),
					None => Access::Fields(vec![name]),
				}),
		))
		.boxed();

		// array subscripts
		let no_start_range = just(Token::DotDot)
			.ignore_then(expr.clone().or_not())
			.map(|end| Subscript::Slice(None, end));
		let with_start = expr
			.clone()
			.then(just(Token::DotDot).ignore_then(expr.clone().or_not()).or_not())
			.map(|(e, extra)| match (e, extra) {
				// closed range
				((Expr::Range { start, end }, _), None) => Subscript::Slice(start.map(|s| *s), end.map(|e| *e)),
				// open range
				(e, Some(end)) => Subscript::Slice(Some(e), end),
				// numeric index
				(e, None) => Subscript::Index(e),
			});
		let subscript = bracket(no_start_range.or(with_start)).boxed();

		// infix operator builder
		let binop = |prec, tok: Token, op: BinOp| {
			infix(left(prec), just(tok), move |l, _, r, ex| {
				(Expr::Binary(op, Box::new(l), Box::new(r)), ex.span())
			})
		};

		let core = atom
			.pratt((
				// field/tuple/method access
				postfix(9, just(Token::Dot).ignore_then(access), |lhs, acc, ex| match acc {
					Access::Fields(parts) => parts.into_iter().fold(lhs, |cur, field| {
						(
							Expr::Field {
								tuple: Box::new(cur),
								field,
							},
							ex.span(),
						)
					}),
					Access::Method(method, args) => (
						Expr::MethodCall {
							recv: Box::new(lhs),
							method,
							args,
						},
						ex.span(),
					),
				}),
				// indexing and slicing
				postfix(9, subscript, |lhs, sub, ex| {
					let collection = Box::new(lhs);
					let e = match sub {
						Subscript::Index(index) => Expr::Index {
							collection,
							index: Box::new(index),
						},
						Subscript::Slice(start, end) => Expr::Slice {
							collection,
							start: start.map(Box::new),
							end: end.map(Box::new),
						},
					};
					(e, ex.span())
				}),
				// propagator
				postfix(9, just(Token::Question), |lhs, _, ex| {
					(Expr::Propagate(Box::new(lhs)), ex.span())
				}),
				// unary
				prefix(8, just(Token::Minus), |_, rhs, ex| {
					(Expr::Negative(Box::new(rhs)), ex.span())
				}),
				prefix(
					8,
					just(Token::Not).then_ignore(result_shape.clone().not()),
					|_, rhs, ex| (Expr::Not(Box::new(rhs)), ex.span()),
				),
				// arithmetic
				binop(7, Token::Asterisk, BinOp::Mul),
				binop(7, Token::Slash, BinOp::Div),
				binop(7, Token::Percent, BinOp::Mod),
				binop(6, Token::Plus, BinOp::Add),
				binop(6, Token::Minus, BinOp::Sub),
				// relational
				binop(5, Token::Lt, BinOp::Lt),
				binop(5, Token::Gt, BinOp::Gt),
				binop(5, Token::Le, BinOp::Le),
				binop(5, Token::Ge, BinOp::Ge),
				// trait check
				postfix(
					5,
					just(Token::Is)
						.ignore_then(just(Token::Ident("not".into())).or_not())
						.then(ident()),
					|lhs, (not, trait_name): (Option<Token>, String), ex| {
						(
							Expr::Is {
								subject: Box::new(lhs),
								trait_name,
								negated: not.is_some(),
							},
							ex.span(),
						)
					},
				),
				// equality | membership
				binop(4, Token::Eq, BinOp::Eq),
				binop(4, Token::Ne, BinOp::Ne),
				binop(4, Token::In, BinOp::In),
				// logical
				binop(3, Token::AndAnd, BinOp::And),
				binop(2, Token::OrOr, BinOp::Or),
				// ranges
				infix(left(1), just(Token::DotDot), |l, _, r, ex| {
					(
						Expr::Range {
							start: Some(Box::new(l)),
							end: Some(Box::new(r)),
						},
						ex.span(),
					)
				}),
				// pipelines
				infix(left(0), just(Token::Pipeline), |l, _, r, ex| pipe(l, r, ex.span())),
			))
			.boxed();

		// juxts (leading literals and trailing functions)
		let trailing = anon_fn.clone().or(block.clone().map_with(|body, ex| {
			(
				Expr::AnonFn {
					captures: None,
					params: vec![],
					params_tuple: true,
					ret: None,
					body,
				},
				ex.span(),
			)
		}));
		let lit_arg = spanned(literal);
		let juxt = choice((
			lit_arg.then(trailing.clone().or_not()).map(|(l, t)| (Some(l), t)),
			trailing.map(|t| (None, Some(t))),
		));
		let juxted = core
			.clone()
			.then(juxt.or_not())
			.try_map(|((inner, s), jx), span| {
				let Some((lit, trail)) = jx else { return Ok((inner, s)) };
				let has_lit = lit.is_some();
				let args: Vec<_> = lit.into_iter().chain(trail).collect();
				let e = match inner {
					Expr::Ident(name) => Expr::Call {
						name,
						type_args: vec![],
						args,
					},
					Expr::Field { tuple, field } => Expr::MethodCall {
						recv: tuple,
						method: field,
						args,
					},
					Expr::Call {
						name,
						type_args,
						args: mut a,
					} if !has_lit => {
						a.extend(args);
						Expr::Call {
							name,
							type_args,
							args: a,
						}
					}
					Expr::MethodCall {
						recv,
						method,
						args: mut a,
					} if !has_lit => {
						a.extend(args);
						Expr::MethodCall { recv, method, args: a }
					}
					_ => return Err(Rich::custom(span, "trailing arg needs a call or method callee")),
				};
				Ok((e, span))
			})
			.or(core.clone())
			.boxed();

		// or blocks
		let or_tail = just(Token::Or).ignore_then(block.clone().or(core.clone().map(|e| vec![pipe_step(e)])));
		header_expr.define(
			core.then(or_tail.clone().or_not())
				.map_with(|pair, ex| or_else(pair, ex.span())),
		);
		juxted
			.then(or_tail.or_not())
			.map_with(|pair, ex| or_else(pair, ex.span()))
			.boxed()
	};
	expr.define(definition);

	// fn defs
	let fn_head = just(Token::Fn).ignore_then(ident()).then(type_params.clone());
	let func = fn_head
		.clone()
		.then(params.clone())
		.then(ret.clone())
		.then(block.clone())
		.map_with(|(((head, params), ret), body), ex| fn_def(head, Some(params), ret, body, ex.span()))
		// pipeline shorthand
		.or(fn_head
			.then(params.clone().or_not())
			.then(ret.clone())
			.then_ignore(just(Token::Assign))
			.then(expr.clone())
			.map_with(|(((head, params), ret), body), ex| {
				fn_def(head, params, ret, vec![dollar_pipe(body)], ex.span())
			}))
		.boxed();

	// struct defs
	let struct_field = ident()
		.then(type_expr.clone())
		.then(just(Token::Assign).ignore_then(expr.clone()).or_not())
		.map_with(|((name, typ), default), ex| Param {
			name,
			typ,
			span: ex.span(),
			default,
			mutable: false,
		})
		.boxed();
	let struct_def = just(Token::Struct)
		.ignore_then(ident())
		.then(type_params.clone())
		.then(brace(loose_list(struct_field.clone())))
		.map_with(|((name, type_params), fields), ex| {
			(
				Expr::StructDef {
					name,
					type_params,
					fields,
				},
				ex.span(),
			)
		})
		.boxed();

	// tuple struct defs
	let ts_field = ident()
		.then_ignore(just(Token::Colon))
		.then(type_expr.clone())
		.map(|(n, t)| (Some(n), t))
		.or(type_expr.clone().map(|t| (None, t)));
	let tuple_struct_def = just(Token::Struct)
		.ignore_then(ident())
		.then(paren(
			ts_field
				.separated_by(just(Token::Comma))
				.allow_trailing()
				.at_least(1)
				.collect::<Vec<_>>(),
		))
		.map_with(|(name, fields), ex| {
			let typ = TypeExpr::TupleStruct(name.clone(), fields);
			(Expr::TypeAlias { name, typ }, ex.span())
		})
		.boxed();

	// enum defs
	let disc_int = just(Token::Minus)
		.or_not()
		.then(select! { Token::Int(n) => n })
		.map(|(neg, n)| (Some(if neg.is_some() { -n } else { n }), None));
	let disc = just(Token::Assign).ignore_then(disc_int.or(select! { Token::String(s) => (None, Some(s)) }));
	let fields = brace(loose_list(ident().then(annot.clone())));
	let backing = just(Token::Colon).ignore_then(annot.clone()).or_not();
	let payload = paren(annot.separated_by(just(Token::Comma)).allow_trailing().collect::<Vec<_>>());
	let variant = ident()
		.then(
			payload
				.map(|p| (vec![], p))
				.or(fields.map(|fs| fs.into_iter().unzip()))
				.or_not(),
		)
		.then(disc.or_not())
		.map(|((name, body), assign)| {
			let (names, payload) = body.unwrap_or_default();
			let (disc, raw) = assign.unwrap_or((None, None));
			EnumVariant {
				name,
				disc,
				raw,
				payload,
				names,
			}
		});
	let enum_def = just(Token::Enum)
		.ignore_then(ident())
		.then(type_params.clone())
		.then(backing)
		.then(brace(loose_list(variant)))
		.try_map_with(|(((name, type_params), backing), variants), ex| {
			let mut next = 0;
			let mut seen = Vec::new();
			for v in &variants {
				let d = v.disc.unwrap_or(next);
				if seen.contains(&d) {
					let msg = format!("discriminant value `{d}` assigned more than once");
					return Err(Rich::custom(ex.span(), msg));
				}
				seen.push(d);
				next = d + 1;
			}
			if backing.is_none() && variants.iter().any(|v| v.raw.is_some()) {
				return Err(Rich::custom(ex.span(), "a raw value needs a string backing"));
			}
			Ok((
				Expr::EnumDef {
					name,
					backing,
					type_params,
					variants,
				},
				ex.span(),
			))
		})
		.boxed();

	// type aliases
	let type_alias = just(Token::Type)
		.ignore_then(ident())
		.then_ignore(just(Token::Assign))
		.then(type_expr.clone())
		.map_with(|(name, typ), ex| (Expr::TypeAlias { name, typ }, ex.span()));

	// trait definitions
	let trait_method = just(Token::Fn)
		.ignore_then(ident())
		.then(params.clone())
		.then(ret.clone())
		.then(block.clone().or_not())
		.map_with(|(((name, params), ret), body), ex| {
			fn_def((name, vec![]), Some(params), ret, body.unwrap_or_default(), ex.span())
		});
	let trait_def = just(Token::Trait)
		.ignore_then(ident())
		.then(
			just(Token::Is)
				.ignore_then(list(ident()))
				.or_not()
				.map(Option::unwrap_or_default),
		)
		.then(brace(
			loose_list(struct_field.clone()).then(trait_method.repeated().collect::<Vec<_>>()),
		))
		.map_with(|((name, supers), (fields, methods)), ex| {
			(
				Expr::TraitDef {
					name,
					supers,
					fields,
					methods,
				},
				ex.span(),
			)
		})
		.boxed();

	let impl_block = just(Token::Impl)
		.ignore_then(ident())
		.then(just(Token::For).ignore_then(ident()).or_not())
		.then(type_params.clone())
		.then(brace(func.clone().repeated().collect::<Vec<_>>()).or_not())
		.map_with(|(((head, target), type_params), methods), ex| {
			let (typ, trait_name) = match target {
				Some(t) => (t, Some(head)),
				None => (head, None),
			};
			(
				Expr::Impl {
					typ,
					type_params,
					trait_name,
					methods: methods.unwrap_or_default(),
				},
				ex.span(),
			)
		})
		.boxed();

	tuple_struct_def
		.or(struct_def)
		.or(enum_def)
		.or(type_alias)
		.or(func)
		.or(trait_def)
		.or(impl_block)
		.or(stmt)
		.repeated()
		.collect()
		.then_ignore(end())
}
