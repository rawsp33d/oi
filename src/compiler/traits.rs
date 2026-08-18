//! Trait declarations and impls checking.

use super::*;

// A trait's supertraits, fields, and methods.
pub(crate) type TraitItem<'a> = (&'a [String], &'a [Param], &'a [Spanned<Expr>]);

// A trait method's name, params, and return annotation.
pub(crate) type TraitFn<'a> = (&'a str, &'a [Param], &'a Option<Spanned<TypeExpr>>);

// A fill's params, tuple-ness, and return annotation.
pub(crate) type FillSig = (Vec<Param>, bool, Option<Spanned<TypeExpr>>);

// A trait impl body.
pub(crate) struct TraitBody<'a> {
	pub span: Span,
	pub typ: &'a str,
	pub trait_name: &'a str,
	pub methods: &'a [Spanned<Expr>],
	pub scope: &'a Scope,
}

// Whether a primitive natively satisfies prelude trait `tn`.
pub(crate) fn builtin_claim(typ: &Typ, tn: &str) -> bool {
	use Typ::*;
	match typ {
		Int(_) => true,
		UInt(_) | ISize | USize => tn != "Neg",
		Float(_) => tn != "Mod",
		Bool | Atom => matches!(tn, "Eq" | "Ord"),
		Str => matches!(tn, "Eq" | "Add"),
		_ => false,
	}
}

pub(crate) fn trait_fns(methods: &[Spanned<Expr>]) -> impl Iterator<Item = TraitFn<'_>> {
	methods.iter().filter_map(|m| match &m.0 {
		Expr::Fn { name, params, ret, .. } => Some((name.as_str(), params.as_slice(), ret)),
		_ => None,
	})
}

// Complete a fill's signature from the trait's declaration.
// An empty, non-tuple param list means the header was omitted entirely.
pub(crate) fn fill_from_decl(
	params: &[Param],
	params_tuple: bool,
	ret: &Option<Spanned<TypeExpr>>,
	(_, dps, dret): TraitFn,
	span: Span,
) -> Result<FillSig, Diagnostic> {
	let ret = ret.clone().or_else(|| dret.clone());
	if params.is_empty() && !params_tuple {
		let ps = (dps.iter().enumerate())
			.map(|(i, d)| {
				let name = if d.name == "self" {
					d.name.clone()
				} else {
					format!("${i}")
				};
				Param {
					name,
					span,
					default: None,
					..d.clone()
				}
			})
			.collect();
		return Ok((ps, dps.len() != 1, ret));
	}
	let omitted = |p: &Param| matches!(&p.typ, TypeExpr::Name(n) if n == "$?");
	// a spelled-out header of the wrong arity is left for the signature check to report
	if params.len() != dps.len() && params.iter().any(omitted) {
		let msg = format!("this fn literal expects {} param(s), got {}", dps.len(), params.len());
		return Err(Diagnostic::new(msg, span.into_range()).with_label("wrong number of params"));
	}
	let ps = (params.iter().enumerate())
		.map(|(i, p)| match dps.get(i) {
			Some(d) if omitted(p) => Param {
				typ: d.typ.clone(),
				..p.clone()
			},
			_ => p.clone(),
		})
		.collect();
	Ok((ps, params_tuple, ret))
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
	let mut defaults: HashMap<(String, String), &str> = HashMap::new();
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
			let Expr::Fn {
				name,
				params,
				params_tuple,
				ret,
				..
			} = &m.0
			else {
				continue;
			};
			let Some(decl @ (_, tp, tr)) = trait_fns(tmethods).find(|(n, ..)| n == name) else {
				continue;
			};
			let (params, _, ret) = fill_from_decl(params, *params_tuple, ret, decl, m.1)?;
			let (got, want) = (sig(&params, &ret)?, sig(tp, tr)?);
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
			// check whether an overlapping fill is already present
			let key = (typ.to_string(), name.clone());
			if !defaults.contains_key(&key)
				&& let Some(f) = others.iter().find(|f| f.key == format!("{typ}.{name}"))
			{
				let (got, want) = (sig(&f.params, &f.ret)?, sig(params, ret)?);
				if got != want {
					let msg = format!("`{typ}.{name}` is `{got}`, trait `{tn}` declares `{want}`");
					return Err(Diagnostic::new(msg, span.into_range()).with_label("wrong signature"));
				}
				continue;
			}
			if body.is_empty() {
				let msg = format!("`{typ}` is missing method `{name}` required by trait `{tn}`");
				return Err(Diagnostic::new(msg, span.into_range()).with_label("provide this method"));
			}
			if let Some(prev) = defaults.insert(key, tn)
				&& prev != tn
			{
				let msg = format!("`{typ}` takes default `{name}` from both `{prev}` and `{tn}`");
				return Err(Diagnostic::new(msg, span.into_range())
					.with_label(format!("fill `{name}` on `{typ}` to settle it")));
			}
			others.push(FnItem {
				key: format!("{typ}.{name}"),
				scope,
				params: params.clone(),
				params_tuple: *params_tuple,
				ret: ret.clone(),
				body,
			});
		}
	}
	Ok(())
}
