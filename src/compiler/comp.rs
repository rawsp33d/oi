// Compile-time evaluation.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};

use crate::ast::Child::{List, One};
use crate::ast::{Expr, Spanned};
use crate::diagnostics::Diagnostic;
use crate::loader::{Module, Program};
use crate::runtime;

use super::Compiler;

pub(crate) const RT_COMP_YIELD: &str = "oi_rt_comp_yield";
pub(crate) const UNREIFIABLE: &str = "can't use this type in `comp` yet";

pub(crate) const TAG_INT: i64 = 0;
pub(crate) const TAG_FLOAT: i64 = 1;
pub(crate) const TAG_BOOL: i64 = 2;
pub(crate) const TAG_STR: i64 = 3;
pub(crate) const TAG_UNIT: i64 = 4;

thread_local! {
	static RESULT: Cell<(i64, i64)> = const { Cell::new((0, 0)) };
}

// Stores the yielded value for `eval` to pick up later.
pub(crate) extern "C" fn rt_comp_yield(tag: i64, val: i64) {
	RESULT.with(|c| c.set((tag, val)));
}

// Find the first not-yet-evaluated `comp` site.
fn find_comp(e: &mut Expr) -> Option<*mut Expr> {
	if matches!(e, Expr::Comp(_)) {
		return Some(e as *mut Expr);
	}
	let mut found = None;
	e.for_children(|c| {
		if found.is_none() {
			found = match c {
				List(list) => list.iter_mut().find_map(|(e, _)| find_comp(e)),
				One((e, _)) => find_comp(e),
			};
		}
	});
	found
}

// Definitions and consts that may be needed.
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

fn reify(tag: i64, val: i64) -> Expr {
	match tag {
		TAG_INT => Expr::Int(val),
		TAG_FLOAT => Expr::Float(f64::from_bits(val as u64)),
		TAG_BOOL => Expr::Bool(val != 0),
		// SAFETY: `val` is a str handle the runtime just produced for this call
		TAG_STR => Expr::String(String::from_utf8_lossy(unsafe { runtime::str_bytes(val as *const _) }).into_owned()),
		TAG_UNIT => Expr::Tuple(vec![]),
		_ => unreachable!("unknown comp tag"),
	}
}

// Evaluate every `comp` site in the main module.
pub(crate) fn eval(expanded: &mut HashMap<String, Vec<Spanned<Expr>>>, program: &Program) -> Result<(), Diagnostic> {
	let scope = program
		.modules
		.iter()
		.find(|m| m.name == "main")
		.expect("a `main` module always exists")
		.scope
		.clone();
	let main_items = expanded.get_mut("main").expect("a `main` module always exists");
	loop {
		let mut items: Vec<Spanned<Expr>> = main_items
			.iter_mut()
			.filter_map(|it| (is_def(&it.0) && find_comp(&mut it.0).is_none()).then(|| it.clone()))
			.collect();
		let Some(ptr) = main_items.iter_mut().find_map(|(e, _)| find_comp(e)) else {
			break;
		};
		// SAFETY: `ptr` points inside `main_items`, which nothing below borrows again
		let (inner, span) = unsafe {
			let Expr::Comp(inner) = &*ptr else {
				unreachable!("find_comp only returns Comp nodes")
			};
			(inner.as_ref().clone(), inner.1)
		};
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
			publics: HashSet::new(),
			reexports: HashMap::new(),
			consts: HashMap::new(),
			annotations: HashMap::new(),
		};
		let mut compiler = Compiler::default();
		compiler.compile(&synthetic)?;
		let f = compiler.module.get_finalized_function(compiler.hoisted["__comp"].id);
		// SAFETY: takes no args and returns unit
		unsafe { std::mem::transmute::<*const u8, fn()>(f)() };
		let (tag, val) = RESULT.with(Cell::get);
		// SAFETY: `ptr` is still valid, we only patch the node it points to
		unsafe { *ptr = reify(tag, val) };
	}
	Ok(())
}
