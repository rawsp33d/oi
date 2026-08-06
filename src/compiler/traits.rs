//! Trait declarations and impls checking.

use super::*;

// A trait's supertraits, fields, and methods.
pub(crate) type TraitItem<'a> = (&'a [String], &'a [Param], &'a [Spanned<Expr>]);

// A trait method's name, params, and return annotation.
pub(crate) type TraitFn<'a> = (&'a str, &'a [Param], &'a Option<Spanned<TypeExpr>>);

// A trait impl body.
pub(crate) struct TraitBody<'a> {
	pub span: Span,
	pub typ: &'a str,
	pub trait_name: &'a str,
	pub methods: &'a [Spanned<Expr>],
	pub scope: &'a Scope,
}

pub(crate) fn trait_fns(methods: &[Spanned<Expr>]) -> impl Iterator<Item = TraitFn<'_>> {
	methods.iter().filter_map(|m| match &m.0 {
		Expr::Fn { name, params, ret, .. } => Some((name.as_str(), params.as_slice(), ret)),
		_ => None,
	})
}

// Check trait impl bodies.
// Validates supertraits, required fields, method sigs.
pub(super) fn check_impls<'p>(
	trait_bodies: Vec<TraitBody<'p>>,
	traits: &HashMap<&'p str, TraitItem<'p>>,
	trait_impls: &HashSet<(String, String)>,
	types: TypeCtx,
	others: &mut Vec<FnItem<'p>>,
) -> Result<(), Diagnostic> {
	for TraitBody {
		span,
		typ,
		trait_name: tn,
		methods,
		scope,
	} in trait_bodies
	{
		if tn == "Drop" {
			let well_formed = methods.iter().any(|m| {
				matches!(&m.0, Expr::Fn { name, params, .. }
					if name == "drop" && params.len() == 1 && params[0].name == "self" && params[0].mutable)
			});
			if !well_formed {
				let msg = format!("`impl Drop for {typ}` must define `fn drop(mut self)`");
				return Err(Diagnostic::new(msg, span.into_range()).with_label("missing or wrong `drop` method"));
			}
			continue;
		}
		let Some((supers, tfields, tmethods)) = traits.get(tn) else {
			return Err(Diagnostic::new(format!("unknown trait `{tn}`"), span.into_range()).with_label("no such trait"));
		};
		for s in *supers {
			if !trait_impls.contains(&(typ.to_string(), s.clone())) {
				let msg = format!("`{typ}` must also implement `{s}`, the supertrait of `{tn}`");
				return Err(Diagnostic::new(msg, span.into_range()).with_label("missing supertrait impl"));
			}
		}
		for tf in *tfields {
			let want = types.resolve(&tf.typ, tf.span)?;
			if !types
				.structs
				.get(typ)
				.is_some_and(|fs| fs.iter().any(|f| f.name == tf.name && f.typ == want))
			{
				let msg = format!("`{typ}` is missing field `{} {want}` required by trait `{tn}`", tf.name);
				return Err(Diagnostic::new(msg, span.into_range()).with_label("required by the trait"));
			}
		}
		let mut sig_aliases = types.aliases.clone();
		sig_aliases.insert("Self".into(), TypeExpr::Name(typ.into()));
		let sig_types = TypeCtx::new(
			types.structs,
			types.enums,
			&sig_aliases,
			types.type_params,
			types.generics,
			types.traits,
		)
		.with_scope(scope);
		let sig = |ps: &[Param], ret: &Option<Spanned<TypeExpr>>| -> Result<Typ, Diagnostic> {
			let params = ps.iter().map(|p| sig_types.resolve(&p.typ, p.span)).collect::<Result<_, _>>()?;
			let ret = match ret {
				Some((te, sp)) => sig_types.resolve(te, *sp)?,
				None => Typ::unit(),
			};
			Ok(Typ::Fn(params, Box::new(ret)))
		};
		for m in methods {
			let Expr::Fn { name, params, ret, .. } = &m.0 else {
				continue;
			};
			let Some((_, tp, tr)) = trait_fns(tmethods).find(|(n, ..)| n == name) else {
				let msg = format!("trait `{tn}` has no method `{name}`");
				return Err(Diagnostic::new(msg, m.1.into_range()).with_label("not in the trait"));
			};
			let (got, want) = (sig(params, ret)?, sig(tp, tr)?);
			if got != want {
				let msg = format!("`{typ}.{name}` is `{got}`, trait `{tn}` declares `{want}`");
				return Err(Diagnostic::new(msg, m.1.into_range()).with_label("wrong signature"));
			}
		}
		for t in *tmethods {
			let Expr::Fn {
				name,
				params,
				params_tuple,
				ret,
				body,
				..
			} = &t.0
			else {
				continue;
			};
			if methods.iter().any(|m| matches!(&m.0, Expr::Fn { name: n, .. } if n == name)) {
				continue;
			}
			if body.is_empty() {
				let msg = format!("`{typ}` is missing method `{name}` required by trait `{tn}`");
				return Err(Diagnostic::new(msg, span.into_range()).with_label("provide this method"));
			}
			others.push(FnItem {
				key: format!("{typ}.{name}"),
				scope,
				params,
				params_tuple: *params_tuple,
				ret,
				body,
			});
		}
	}
	Ok(())
}
