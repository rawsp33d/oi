// Compile-time evaluation.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::ast::{Annotation, Expr, Span, Spanned};
use crate::diagnostics::Diagnostic;
use crate::loader::{Module, Program, Scope};
use crate::runtime;

use super::Compiler;

pub(crate) const RT_COMP_YIELD: &str = "oi_rt_comp_yield";
pub(crate) const RT_COMP_STRUCT: &str = "oi_rt_comp_struct";
pub(crate) const UNREIFIABLE: &str = "can't use this type in `comp` yet";

pub(crate) const TAG_INT: i64 = 0;
pub(crate) const TAG_FLOAT: i64 = 1;
pub(crate) const TAG_BOOL: i64 = 2;
pub(crate) const TAG_STR: i64 = 3;
pub(crate) const TAG_UNIT: i64 = 4;

enum Entry {
	Scalar(i64, i64),
	Struct(String, usize),
}

thread_local! {
	static STACK: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
}

pub(crate) extern "C" fn rt_comp_yield(tag: i64, val: i64) {
	STACK.with_borrow_mut(|s| s.push(Entry::Scalar(tag, val)));
}

pub(crate) extern "C" fn rt_comp_struct(name: *const runtime::StrHeader, nfields: i64) {
	// SAFETY: `name` is a str constant the JIT just built
	let name = String::from_utf8_lossy(unsafe { runtime::str_bytes(name) }).into_owned();
	STACK.with_borrow_mut(|s| s.push(Entry::Struct(name, nfields as usize)));
}

// Find the first unevaluated comp site.
fn first_comp(items: &mut [Spanned<Expr>]) -> Option<Spanned<Expr>> {
	let mut found = None;
	for (e, _) in items.iter_mut() {
		e.walk(&mut |x| {
			if found.is_none()
				&& let Expr::Comp(inner) = x
			{
				found = Some((**inner).clone());
			}
		});
	}
	found
}

// Swap comp site for its folded literal.
fn patch_first(items: &mut [Spanned<Expr>], lit: Expr) {
	let mut lit = Some(lit);
	for (e, _) in items.iter_mut() {
		e.walk(&mut |x| {
			if matches!(x, Expr::Comp(_))
				&& let Some(lit) = lit.take()
			{
				*x = lit;
			}
		});
	}
}

fn has_comp(e: &mut Expr) -> bool {
	let mut found = false;
	e.walk(&mut |x| found |= matches!(x, Expr::Comp(_)));
	found
}

fn is_def(e: &Expr) -> bool {
	matches!(
		e,
		Expr::Fn { .. }
			| Expr::StructDef { .. }
			| Expr::EnumDef { .. }
			| Expr::TraitDef { .. }
			| Expr::TypeAlias { .. }
			| Expr::Claim { .. }
			| Expr::Bind { .. }
	)
}

// Decode the stack.
fn reify(span: Span) -> Expr {
	STACK
		.with_borrow_mut(|stack| {
			let mut exprs: Vec<Expr> = Vec::new();
			for entry in stack.drain(..) {
				let e = match entry {
					Entry::Scalar(TAG_INT, v) => Expr::Int(v),
					Entry::Scalar(TAG_FLOAT, v) => Expr::Float(f64::from_bits(v as u64)),
					Entry::Scalar(TAG_BOOL, v) => Expr::Bool(v != 0),
					// SAFETY: `v` is a str handle the runtime just produced
					Entry::Scalar(TAG_STR, v) => {
						Expr::String(String::from_utf8_lossy(unsafe { runtime::str_bytes(v as *const _) }).into_owned())
					}
					Entry::Scalar(..) => Expr::Tuple(vec![]),
					Entry::Struct(name, nfields) => {
						let at = exprs.len() - nfields;
						let fields = exprs.split_off(at).into_iter().map(|e| (None, (e, span))).collect();
						Expr::StructLit {
							name,
							type_args: vec![],
							fields,
						}
					}
				};
				exprs.push(e);
			}
			exprs.pop()
		})
		.expect("a comp site always yields exactly one value")
}

// Compile and run a single comp site, folding it to its reified literal.
fn fold(
	inner: Spanned<Expr>,
	main_items: &mut [Spanned<Expr>],
	scope: &Scope,
	program: &Program,
) -> Result<Expr, Diagnostic> {
	let span = inner.1;
	let mut items: Vec<Spanned<Expr>> = main_items
		.iter_mut()
		.filter_map(|it| (is_def(&it.0) && !has_comp(&mut it.0)).then(|| it.clone()))
		.collect();
	items.push((
		Expr::Fn {
			name: "__comp".into(),
			type_params: vec![],
			params: vec![],
			params_tuple: true,
			ret: None,
			body: vec![(
				Expr::Call {
					name: "__comp_yield".into(),
					type_args: vec![],
					args: vec![inner],
				},
				span,
			)],
		},
		span,
	));
	let synthetic = Program {
		map: program.map.clone(),
		modules: vec![Module {
			name: "main".into(),
			items,
			scope: scope.clone(),
		}],
		..Program::default()
	};
	let mut compiler = Compiler::default();
	compiler.compile(&synthetic)?;
	let f = compiler.module.get_finalized_function(compiler.hoisted["__comp"].id);
	// SAFETY: fn takes no args and returns unit
	unsafe { std::mem::transmute::<*const u8, fn()>(f)() };
	Ok(reify(span))
}

// Fold every `comp` site in the main module, and every call in annotation position, to a literal.
pub(crate) fn eval(
	expanded: &mut HashMap<String, Vec<Spanned<Expr>>>,
	annotations: &mut HashMap<String, Vec<Annotation>>,
	program: &Program,
) -> Result<(), Diagnostic> {
	let main = program
		.modules
		.iter()
		.find(|m| m.name == "main")
		.expect("a `main` module always exists");
	let main_items = expanded.get_mut("main").expect("a `main` module always exists");
	while let Some(inner) = first_comp(main_items) {
		let lit = fold(inner, main_items, &main.scope, program)?;
		patch_first(main_items, lit);
	}
	// an annotation call is implicitly comptime
	for a in annotations.iter_mut().filter(|(k, _)| !k.contains("::")).flat_map(|(_, v)| v) {
		if matches!(a.0, Expr::Call { .. }) {
			a.0 = fold(a.clone(), main_items, &main.scope, program)?;
		}
	}
	Ok(())
}
