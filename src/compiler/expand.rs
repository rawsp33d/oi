// User-defined macros.

use std::collections::{HashMap, HashSet};

use crate::ast::{Capture, Child, Expr, Param, Pattern, Span, Spanned, TypeExpr};
use crate::diagnostics::Diagnostic;

use Child::{List, One};

const BUILTINS: [&str; 5] = ["dbg", "assert", "panic", "todo", "unreachable"];
const MAX_DEPTH: usize = 64;

fn fail<T>(msg: impl Into<String>, span: Span, label: &str) -> Result<T, Diagnostic> {
	Err(Diagnostic::new(msg, span.into_range()).with_label(label))
}

struct Macro {
	params: Vec<String>,
	bound: HashSet<String>,
	body: Vec<Spanned<Expr>>,
}

#[derive(Default)]
struct Expander {
	macros: HashMap<String, Macro>,
	counter: usize,
}

// Visit every name-binding site in expr.
// Skips deliberate `%name` captures.
fn for_binders(e: &mut Expr, f: &mut impl FnMut(&mut String)) {
	match e {
		Expr::Bind { name, .. } if !name.starts_with('%') => f(name),
		Expr::Destructure { names, bind: true, .. } => names.iter_mut().for_each(|(_, n)| f(n)),
		Expr::For {
			pat: Pattern::Name(n), ..
		} => f(n),
		Expr::For {
			pat: Pattern::Tuple(ns),
			..
		} => ns.iter_mut().for_each(f),
		Expr::AnonFn { params, .. } => params.iter_mut().for_each(|p| f(&mut p.name)),
		Expr::Match { arms, .. } => arms.iter_mut().flat_map(|a| &mut a.binding).for_each(f),
		_ => {}
	}
}

// One pass over a template copy.
// Adds suffix to the template's own binders for good hygiene (see what I did there?).
// Splices `%name` arguments in verbatim.
fn fill(e: &mut Spanned<Expr>, m: &Macro, args: &[Spanned<Expr>], suffix: usize) -> Result<(), Diagnostic> {
	let idx = |name: &str, span: Span| {
		m.params.iter().position(|p| p == name).ok_or_else(|| {
			Diagnostic::new(format!("no macro param named `{name}`"), span.into_range()).with_label("unknown param")
		})
	};
	if let Expr::Unquote(name) = &e.0 {
		*e = args[idx(name, e.1)?].clone();
		return Ok(());
	}
	let rename = &mut |n: &mut String| {
		if m.bound.contains(n.as_str()) {
			*n = format!("{n}#{suffix}");
		}
	};
	for_binders(&mut e.0, rename);
	match &mut e.0 {
		Expr::Ident(n)
		| Expr::Assign { name: n, .. }
		| Expr::Call { name: n, .. }
		| Expr::FieldAssign { name: n, .. }
		| Expr::IndexAssign { name: n, .. }
		| Expr::Append { name: n, .. }
		| Expr::MapDelete { name: n, .. } => rename(n),
		Expr::Destructure { names, bind: false, .. } => names.iter_mut().for_each(|(_, n)| rename(n)),
		Expr::AnonFn {
			captures: Some(list), ..
		} => {
			for c in list {
				let (Capture::ReadOnly(n) | Capture::Mut(n) | Capture::Move(n)) = c;
				rename(n);
			}
		}
		_ => {}
	}
	if let Expr::Bind { name, .. } = &mut e.0
		&& let Some(param) = name.strip_prefix('%')
	{
		let i = idx(param, e.1)?;
		let Expr::Ident(n) = &args[i].0 else {
			return fail("a `%name` binder needs a plain name argument", args[i].1, "not a name");
		};
		*name = n.clone();
	}
	e.0.try_children(|c| match c {
		List(list) => list.iter_mut().try_for_each(|i| fill(i, m, args, suffix)),
		One(one) => fill(one, m, args, suffix),
	})
}

impl Expander {
	fn define(
		&mut self,
		name: String,
		params: Vec<Param>,
		body: Vec<Spanned<Expr>>,
		span: Span,
	) -> Result<(), Diagnostic> {
		if BUILTINS.contains(&name.as_str()) {
			return fail(format!("`{name}!` is a builtin macro"), span, "reserved name");
		}
		if self.macros.contains_key(&name) {
			return fail(format!("`{name}!` is already defined"), span, "duplicate macro");
		}
		if let Some(p) = params.iter().find(|p| !matches!(&p.typ, TypeExpr::Name(n) if n == "Ast")) {
			return fail("macro params must be `Ast`", p.span, "not Ast");
		}
		let [(Expr::Quote(stmts), _)] = body.as_slice() else {
			return fail(
				"only template macros (a body of exactly one quote) are supported yet",
				span,
				"macro body",
			);
		};
		let (mut body, mut bound) = (stmts.clone(), HashSet::new());
		for (e, _) in body.iter_mut() {
			e.walk(&mut |e| {
				for_binders(e, &mut |n| {
					bound.insert(n.clone());
				});
			});
		}
		let params = params.into_iter().map(|p| p.name).collect();
		self.macros.insert(name, Macro { params, bound, body });
		Ok(())
	}

	// Instantiate expr if it calls a user macro.
	fn call(&mut self, e: &Spanned<Expr>, depth: usize) -> Result<Option<Vec<Spanned<Expr>>>, Diagnostic> {
		let Expr::MacroCall { name, args } = &e.0 else {
			return Ok(None);
		};
		let Some(def) = self.macros.get(name) else {
			return Ok(None);
		};
		if args.len() != def.params.len() {
			let s = if def.params.len() == 1 { "" } else { "s" };
			let msg = format!("`{name}!` takes {} argument{s}, got {}", def.params.len(), args.len());
			return fail(msg, e.1, "wrong number of arguments");
		}
		if depth >= MAX_DEPTH {
			return fail("macro expansion is too deep", e.1, "recursion limit");
		}
		self.counter += 1;
		let mut body = def.body.clone();
		for item in &mut body {
			fill(item, def, args, self.counter)?;
		}
		self.expand(List(&mut body), depth + 1)?;
		Ok(Some(body))
	}

	// Walk the tree, expanding macro calls.
	fn expand(&mut self, c: Child, depth: usize) -> Result<(), Diagnostic> {
		match c {
			List(body) => {
				let mut i = 0;
				while i < body.len() {
					match self.call(&body[i], depth)? {
						Some(stmts) => {
							let n = stmts.len();
							body.splice(i..i + 1, stmts);
							i += n;
						}
						None => {
							self.expand(One(&mut body[i]), depth)?;
							i += 1;
						}
					}
				}
				Ok(())
			}
			One(e) => {
				if let Some(mut stmts) = self.call(e, depth)? {
					e.0 = match stmts.len() {
						1 => stmts.pop().unwrap().0,
						_ => Expr::Block(stmts),
					};
					return Ok(());
				}
				match &e.0 {
					Expr::Quote(_) => fail("quotes are only allowed as a macro's whole body", e.1, "stray quote"),
					Expr::Unquote(_) => fail("unquotes only make sense inside a macro template", e.1, "stray unquote"),
					Expr::MacroDef { .. } => {
						fail("macros can only be defined at the top level", e.1, "nested macro def")
					}
					_ => e.0.try_children(|c| self.expand(c, depth)),
				}
			}
		}
	}
}

// Expand all user macro calls in a module's items.
pub fn expand(items: Vec<Spanned<Expr>>) -> Result<Vec<Spanned<Expr>>, Diagnostic> {
	let mut ex = Expander::default();
	let mut rest = Vec::with_capacity(items.len());
	for item in items {
		match item.0 {
			Expr::MacroDef { name, params, body } => ex.define(name, params, body, item.1)?,
			_ => rest.push(item),
		}
	}
	ex.expand(List(&mut rest), 0)?;
	Ok(rest)
}
