// User-defined comptime macros.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::ast::{Capture, Child, Expr, Param, Pattern, Span, Spanned, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::loader::{Module, Program};
use crate::runtime;

use super::Compiler;

use Child::{List, One};

const BUILTINS: [&str; 5] = ["dbg", "assert", "panic", "todo", "unreachable"];
const MAX_DEPTH: usize = 64;
const MAX_PARAMS: usize = 4;

pub(crate) const RT_QUOTE: &str = "oi_rt_quote";
pub(crate) const RT_AST_INT: &str = "oi_rt_ast_int";
pub(crate) const RT_AST_INT_VALUE: &str = "oi_rt_ast_int_value";
pub(crate) const RT_AST_ITEMS: &str = "oi_rt_ast_items";

fn fail<T>(msg: impl Into<String>, span: Span, label: &str) -> Result<T, Diagnostic> {
	Err(Diagnostic::new(msg, span.into_range()).with_label(label))
}

#[derive(Default)]
struct Expander {
	// stage-0 fns, in definition order
	defs: Vec<Spanned<Expr>>,
	// name -> (arity, stage-0 fn pointer)
	macros: HashMap<String, (usize, *const u8)>,
	// keeps the stage-0 JIT and its code alive for every call this pass makes
	stage0: Option<Compiler>,
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

// Rewrite direct macro calls from stage-0 bodies into calls of their compiled fns.
// Unquote expressions run at comptime.
// Other calls inside quotes stay as calls and expand later, when the produced Ast is spliced.
fn direct_calls(e: &mut Expr, macros: &HashMap<String, (usize, *const u8)>, in_quote: bool) {
	if !in_quote
		&& let Expr::MacroCall { name, args } = e
		&& macros.contains_key(name.as_str())
	{
		let (name, args) = (format!("{name}!"), std::mem::take(args));
		*e = Expr::Call {
			name,
			type_args: vec![],
			args,
		};
	}
	let child_in_quote = match e {
		Expr::Quote(_) => true,
		Expr::UnquoteExpr(_) | Expr::UnquoteSplat(_) => false,
		_ => in_quote,
	};
	e.for_children(|c| match c {
		List(list) => list.iter_mut().for_each(|(e, _)| direct_calls(e, macros, child_in_quote)),
		One((e, _)) => direct_calls(e, macros, child_in_quote),
	});
}

impl Expander {
	fn define(
		&mut self,
		name: String,
		params: Vec<Param>,
		ret: Option<Spanned<TypeExpr>>,
		body: Vec<Spanned<Expr>>,
		span: Span,
	) -> Result<(), Diagnostic> {
		let ast = |te: &TypeExpr| matches!(te, TypeExpr::Name(n) if n == "Ast");
		if BUILTINS.contains(&name.as_str()) {
			return fail(format!("`{name}!` is a builtin macro"), span, "reserved name");
		}
		if self.macros.contains_key(&name) {
			return fail(format!("`{name}!` is already defined"), span, "duplicate macro");
		}
		if let Some(p) = params.iter().find(|p| !ast(&p.typ)) {
			return fail("macro params must be `Ast`", p.span, "not Ast");
		}
		if params.len() > MAX_PARAMS {
			return fail("macros take at most 4 arguments for now", span, "too many parameters");
		}
		if let Some((te, rspan)) = &ret
			&& !ast(te)
		{
			return fail("macros return `Ast`", *rspan, "not Ast");
		}
		self.macros.insert(name.clone(), (params.len(), std::ptr::null()));
		let f = Expr::Fn {
			name: format!("{name}!"),
			type_params: vec![],
			params_tuple: params.len() != 1,
			params,
			ret: Some((TypeExpr::Name("Ast".into()), (0..0).into())),
			body,
		};
		self.defs.push((f, span));
		Ok(())
	}

	// Compile every macro body as a real fn, in one synthetic program, and grab their pointers.
	fn compile_stage0(&mut self, program: &Program) -> Result<(), Diagnostic> {
		for (e, _) in &mut self.defs {
			direct_calls(e, &self.macros, false);
		}
		let main = program
			.modules
			.iter()
			.find(|m| m.name == "main")
			.expect("a `main` module always exists");
		let synthetic = Program {
			map: program.map.clone(),
			modules: vec![Module {
				name: "main".into(),
				items: std::mem::take(&mut self.defs),
				scope: main.scope.clone(),
			}],
			publics: HashSet::new(),
			reexports: HashMap::new(),
			consts: HashMap::new(),
		};
		let mut compiler = Compiler::default();
		compiler.compile(&synthetic)?;
		for (name, (_, ptr)) in &mut self.macros {
			*ptr = compiler.module.get_finalized_function(compiler.hoisted[&format!("{name}!")].id);
		}
		self.stage0 = Some(compiler);
		Ok(())
	}

	// Instantiate expr if it calls a user macro.
	fn call(&mut self, e: &Spanned<Expr>, depth: usize) -> Result<Option<Vec<Spanned<Expr>>>, Diagnostic> {
		let Expr::MacroCall { name, args } = &e.0 else {
			return Ok(None);
		};
		let Some(&(arity, ptr)) = self.macros.get(name) else {
			return Ok(None);
		};
		if args.len() != arity {
			let s = if arity == 1 { "" } else { "s" };
			let msg = format!("`{name}!` takes {arity} argument{s}, got {}", args.len());
			return fail(msg, e.1, "wrong number of arguments");
		}
		if depth >= MAX_DEPTH {
			return fail("macro expansion is too deep", e.1, "recursion limit");
		}
		type Ptr = *mut Spanned<Expr>;
		let boxed: Vec<Ptr> = args.iter().map(|a| Box::into_raw(Box::new(a.clone()))).collect();
		let arg = |i: usize| boxed.get(i).copied().unwrap_or(std::ptr::null_mut());
		// SAFETY: stage-0 fns take at most MAX_PARAMS pointer args, all in registers on the ABIs cranelift targets, so a fixed-shape call just leaves the extras unread.
		let f = unsafe { std::mem::transmute::<*const u8, fn(Ptr, Ptr, Ptr, Ptr) -> Ptr>(ptr) };
		let (e0, span) = unsafe { *Box::from_raw(f(arg(0), arg(1), arg(2), arg(3))) };
		let mut body = match e0 {
			Expr::Block(stmts) => stmts,
			other => vec![(other, span)],
		};
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
					Expr::Fn { name, .. } if name.contains('!') => Ok(()),
					Expr::Quote(_) => fail("quotes are only allowed inside macro definitions", e.1, "stray quote"),
					Expr::Unquote(_) | Expr::UnquoteExpr(_) | Expr::UnquoteSplat(_) => {
						fail("unquotes only make sense inside a macro template", e.1, "stray unquote")
					}
					Expr::MacroDef { .. } => {
						fail("macros can only be defined at the top level", e.1, "nested macro def")
					}
					_ => e.0.try_children(|c| self.expand(c, depth)),
				}
			}
		}
	}
}

// Expand all macro calls across a program's modules.
pub fn expand(program: &Program) -> Result<HashMap<String, Vec<Spanned<Expr>>>, Diagnostic> {
	let mut ex = Expander::default();
	let mut rest: HashMap<String, Vec<Spanned<Expr>>> = HashMap::new();
	for m in &program.modules {
		let mut items = Vec::with_capacity(m.items.len());
		for item in m.items.iter().cloned() {
			match item.0 {
				Expr::MacroDef {
					name,
					params,
					ret,
					body,
				} => ex.define(name, params, ret, body, item.1)?,
				_ => items.push(item),
			}
		}
		rest.insert(m.name.clone(), items);
	}
	if !ex.macros.is_empty() {
		ex.compile_stage0(program)?;
	}
	for items in rest.values_mut() {
		ex.expand(List(items), 0)?;
	}
	Ok(rest)
}

// A quote template.
struct Template {
	stmts: Vec<Spanned<Expr>>,
	names: Vec<String>,
	bound: HashSet<String>,
}

static HYGIENE: AtomicUsize = AtomicUsize::new(0);

// Macro-run failures can't recover.
fn die(msg: &str) -> ! {
	eprintln!("macro error: {msg}");
	std::process::exit(1)
}

// One unquote site in a template.
pub(crate) enum Slot {
	Name(String),
	Expr(Spanned<Expr>),
	Splat(Spanned<Expr>),
}

// An instantiation argument.
// One Ast, or a splat's `[]Ast` elements.
enum Arg<'a> {
	Ast(&'a Spanned<Expr>),
	Seq(Vec<Spanned<Expr>>),
}

fn push_name(slots: &mut Vec<Slot>, n: &str) {
	if !slots.iter().any(|s| matches!(s, Slot::Name(m) if m == n)) {
		slots.push(Slot::Name(n.to_string()));
	}
}

// Walk a template body collecting unquote slots and binders, flagging nested quotes.
// Unquote expressions are pulled out and replaced with an `Unquote`.
fn scan(e: &mut Expr, slots: &mut Vec<Slot>, bound: &mut HashSet<String>, nested: &mut bool) {
	match e {
		Expr::Quote(_) => *nested = true,
		Expr::Unquote(n) => push_name(slots, n),
		Expr::UnquoteExpr(inner) | Expr::UnquoteSplat(inner) => {
			let taken = std::mem::replace(inner.as_mut(), (Expr::Unquote(String::new()), (0..0).into()));
			let (key, slot) = match e {
				Expr::UnquoteSplat(_) => (format!("...{}", slots.len()), Slot::Splat(taken)),
				_ => (slots.len().to_string(), Slot::Expr(taken)),
			};
			slots.push(slot);
			*e = Expr::Unquote(key);
		}
		Expr::Bind { name, .. } if name.starts_with('%') => push_name(slots, &name[1..]),
		_ => {}
	}
	for_binders(e, &mut |n| {
		bound.insert(n.clone());
	});
	e.for_children(|c| match c {
		List(list) => list.iter_mut().for_each(|(e, _)| scan(e, slots, bound, nested)),
		One((e, _)) => scan(e, slots, bound, nested),
	});
}

// Validate a quote and leak it as a template.
pub(crate) fn register(stmts: &[Spanned<Expr>], span: Span) -> Result<(usize, Vec<Slot>), Diagnostic> {
	let mut stmts = stmts.to_vec();
	let (mut slots, mut bound, mut nested) = (Vec::new(), HashSet::new(), false);
	for (e, _) in &mut stmts {
		scan(e, &mut slots, &mut bound, &mut nested);
	}
	if nested {
		return fail("nested quotes aren't supported yet", span, "nested quote");
	}
	let names = slots
		.iter()
		.enumerate()
		.map(|(i, s)| match s {
			Slot::Name(n) => n.clone(),
			Slot::Expr(_) => i.to_string(),
			Slot::Splat(_) => format!("...{i}"),
		})
		.collect();
	let tpl = Template { names, stmts, bound };
	Ok((Box::into_raw(Box::new(tpl)) as usize, slots))
}

// One pass over a template copy.
// Adds a suffix to the template's own binders for good hygiene (see what I did there?).
// Splices `%name` arguments in verbatim.
fn fill(e: &mut Spanned<Expr>, bound: &HashSet<String>, args: &HashMap<&str, Arg>, suffix: usize) {
	if let Expr::Unquote(name) = &e.0 {
		match &args[name.as_str()] {
			Arg::Ast(v) => *e = (*v).clone(),
			Arg::Seq(_) => die("%{...} spread needs a sequence position"),
		}
		return;
	}
	let rename = &mut |n: &mut String| {
		if bound.contains(n.as_str()) {
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
		let Arg::Ast((Expr::Ident(n), _)) = &args[param] else {
			die("a `%name` binder needs a plain name argument");
		};
		*name = n.clone();
	}
	match &mut e.0 {
		Expr::Call { args: list, .. }
		| Expr::MacroCall { args: list, .. }
		| Expr::EnumShorthand { args: list, .. }
		| Expr::Array(list)
		| Expr::DotArray(_, list)
		| Expr::DotTuple(list) => splice(list, bound, args, suffix),
		Expr::MethodCall { recv, args: list, .. } => {
			fill(recv, bound, args, suffix);
			splice(list, bound, args, suffix);
		}
		_ => e.0.for_children(|c| match c {
			List(list) => splice(list, bound, args, suffix),
			One(one) => fill(one, bound, args, suffix),
		}),
	}
}

// Walk a sequence position, splicing `%{...expr}` slots in verbatim and filling everything else.
fn splice(list: &mut Vec<Spanned<Expr>>, bound: &HashSet<String>, args: &HashMap<&str, Arg>, suffix: usize) {
	let mut i = 0;
	while i < list.len() {
		if let Expr::Unquote(n) = &list[i].0
			&& let Arg::Seq(items) = &args[n.as_str()]
		{
			let items = items.clone();
			let n = items.len();
			list.splice(i..i + 1, items);
			i += n;
		} else {
			fill(&mut list[i], bound, args, suffix);
			i += 1;
		}
	}
}

// Instantiate the template at `tpl`, substituting `args` by position against its unquote names.
// `args` are borrowed, not owned.
pub(crate) extern "C" fn rt_quote(tpl: usize, args: *const *mut Spanned<Expr>, len: usize) -> *mut Spanned<Expr> {
	// SAFETY: `tpl` was leaked by `register`.
	let tpl = unsafe { &*(tpl as *const Template) };
	let args = if len == 0 {
		&[]
	} else {
		unsafe { std::slice::from_raw_parts(args, len) }
	};
	let suffix = HYGIENE.fetch_add(1, Ordering::Relaxed) + 1;
	let map: HashMap<&str, Arg> = tpl
		.names
		.iter()
		.zip(args)
		.map(|(n, &p)| {
			let arg = if n.starts_with("...") {
				// SAFETY: the lowerer passes a `[]Ast` header for splat slots
				let elems = unsafe { runtime::array_elems(p.cast()) };
				Arg::Seq(elems.iter().map(|&q| unsafe { (*(q as *mut Spanned<Expr>)).clone() }).collect())
			} else {
				Arg::Ast(unsafe { &*p })
			};
			(n.as_str(), arg)
		})
		.collect();
	let mut stmts = tpl.stmts.clone();
	splice(&mut stmts, &tpl.bound, &map, suffix);
	let result = match stmts.len() {
		1 => stmts.pop().unwrap(),
		_ => {
			let span = stmts.first().map_or((0..0).into(), |s| s.1);
			(Expr::Block(stmts), span)
		}
	};
	Box::into_raw(Box::new(result))
}

pub(crate) extern "C" fn rt_ast_int(v: i64) -> *mut Spanned<Expr> {
	Box::into_raw(Box::new((Expr::Int(v), (0..0).into())))
}

pub(crate) extern "C" fn rt_ast_int_value(a: *mut Spanned<Expr>) -> i64 {
	match unsafe { &(*a).0 } {
		Expr::Int(n) => *n,
		_ => die("`.int()` needs an Ast holding an Int literal"),
	}
}

// `.items()`: the Asts making up a []Ast.
pub(crate) extern "C" fn rt_ast_items(a: *mut Spanned<Expr>) -> *const runtime::Header {
	let items = match unsafe { &(*a).0 } {
		Expr::Array(v)
		| Expr::DotArray(_, v)
		| Expr::Block(v)
		| Expr::Call { args: v, .. }
		| Expr::MacroCall { args: v, .. } => v,
		_ => die("this Ast has no items"),
	};
	let ptrs: Vec<i64> = items.iter().map(|e| Box::into_raw(Box::new(e.clone())) as i64).collect();
	runtime::array_of(&ptrs)
}
