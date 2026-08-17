use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use cranelift::codegen;
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};

use crate::ast::{EnumVariant, Expr, Param, Span, Spanned, TypeExpr, TypeParam};
use crate::diagnostics::Diagnostic;
use crate::loader::{Program, Scope};
use crate::runtime;

mod lower;
mod resolve;
mod traits;
mod typ;

use lower::Translator;
pub(crate) use resolve::*;
pub(crate) use traits::*;
pub(crate) use typ::*;

struct FnItem<'a> {
	key: String,
	scope: &'a Scope,
	params: &'a [Param],
	params_tuple: bool,
	ret: &'a Option<Spanned<TypeExpr>>,
	body: &'a [Spanned<Expr>],
}

type EnumItem<'a> = (&'a str, Option<&'a Spanned<TypeExpr>>, &'a [EnumVariant]);

#[derive(Clone)]
pub(crate) struct FnSig {
	pub id: FuncId,
	pub params: Vec<Typ>,
	pub muts: Vec<bool>,
	pub ret: Typ,
}

impl FnSig {
	// Params as a fn value sees them, `mut` folded back in.
	pub(crate) fn value_params(&self) -> Vec<Typ> {
		self.params
			.iter()
			.zip(&self.muts)
			.map(|(t, &m)| if m { Typ::Mut(Box::new(t.clone())) } else { t.clone() })
			.collect()
	}
}

// A generic free function, monomorphized per callsite.
#[derive(Clone)]
pub(crate) struct GenericFnDef {
	pub params: Vec<Param>,
	pub params_tuple: bool,
	pub ret: Option<Spanned<TypeExpr>>,
	pub body: Vec<Spanned<Expr>>,
	pub type_params: Vec<TypeParam>,
	pub captures: Vec<(String, Typ, bool)>,
	pub self_name: Option<String>,
}

// A monomorphized instance whose sig is declared but body not yet compiled.
pub(crate) type Pending = (String, GenericFnDef, HashMap<String, Typ>);

// A resolved fn.
#[derive(Default)]
struct FnDef<'a> {
	params: &'a [(String, Typ, bool)],
	params_tuple: bool,
	ret: Option<(Typ, Span)>,
	body: &'a [Spanned<Expr>],
	self_type: Option<&'a str>,
	is_main: bool,
	captures: &'a [(String, Typ, bool)],
	self_fn: Option<(&'a str, &'a FnSig)>,
}

// A generic struct definition.
#[derive(Clone)]
pub(crate) struct GenericStructDef {
	pub type_params: Vec<TypeParam>,
	pub fields: Vec<Param>,
}

// A generic enum definition.
#[derive(Clone)]
pub(crate) struct GenericEnumDef {
	pub type_params: Vec<TypeParam>,
	pub variants: Vec<EnumVariant>,
}

// Generic type definitions.
#[derive(Default)]
pub(crate) struct Generics {
	pub structs: HashMap<String, GenericStructDef>,
	pub enums: HashMap<String, GenericEnumDef>,
	// enum instances keyed by display name (`Opt[int]`)
	pub instances: RefCell<HashMap<String, Vec<VariantInfo>>>,
	// struct instances' concrete type args keyed by display name (`Box[int]`)
	pub instance_args: RefCell<HashMap<String, Vec<Typ>>>,
}

// Does a type ref mention the named type?
fn mentions(te: &TypeExpr, name: &str) -> bool {
	match te {
		TypeExpr::Name(n) => n == name,
		TypeExpr::Array(e) | TypeExpr::FixedArray(e, _) | TypeExpr::Option(e) => mentions(e, name),
		TypeExpr::Result(e, err) => mentions(e, name) || err.as_deref().is_some_and(|e| mentions(e, name)),
		TypeExpr::Tuple(es) | TypeExpr::Sum(es) | TypeExpr::Generic(_, es) => es.iter().any(|e| mentions(e, name)),
		TypeExpr::Fn(ps, _, r) => ps.iter().any(|p| mentions(p, name)) || mentions(r, name),
		TypeExpr::TupleStruct(_, fs) => fs.iter().any(|(_, t)| mentions(t, name)),
		TypeExpr::Map(k, v) => mentions(k, name) || mentions(v, name),
		TypeExpr::Ref(e) => mentions(e, name),
		TypeExpr::AtomSum(_) => false,
	}
}

// No placeholder struct may appear outside a `&T`.
fn ref_guarded(typ: &Typ, placeholders: &HashSet<String>) -> bool {
	match typ {
		Typ::Ref(_) => true,
		Typ::Struct(n, fs) => !placeholders.contains(n) && fs.iter().all(|f| ref_guarded(&f.typ, placeholders)),
		Typ::Option(i) | Typ::Result(i) | Typ::Array(i) | Typ::FixedArray(i, _) => ref_guarded(i, placeholders),
		Typ::Map(k, v) => ref_guarded(k, placeholders) && ref_guarded(v, placeholders),
		Typ::Tuple(fs) | Typ::TupleStruct(_, fs) => fs.iter().all(|(_, t)| ref_guarded(t, placeholders)),
		Typ::Sum(vs) => vs.iter().all(|v| v.payload.iter().all(|t| ref_guarded(t, placeholders))),
		_ => true,
	}
}

// Rewrite `Self` type refs to the owning type.
fn replace_self(te: &TypeExpr, self_ty: &TypeExpr) -> TypeExpr {
	match te {
		TypeExpr::Name(n) if n == "Self" => self_ty.clone(),
		TypeExpr::Array(e) => TypeExpr::Array(Box::new(replace_self(e, self_ty))),
		TypeExpr::FixedArray(e, n) => TypeExpr::FixedArray(Box::new(replace_self(e, self_ty)), *n),
		TypeExpr::Option(e) => TypeExpr::Option(Box::new(replace_self(e, self_ty))),
		TypeExpr::Result(e, err) => TypeExpr::Result(Box::new(replace_self(e, self_ty)), err.clone()),
		TypeExpr::Tuple(es) => TypeExpr::Tuple(es.iter().map(|e| replace_self(e, self_ty)).collect()),
		TypeExpr::Fn(ps, muts, r) => TypeExpr::Fn(
			ps.iter().map(|p| replace_self(p, self_ty)).collect(),
			muts.clone(),
			Box::new(replace_self(r, self_ty)),
		),
		TypeExpr::Map(k, v) => TypeExpr::Map(Box::new(replace_self(k, self_ty)), Box::new(replace_self(v, self_ty))),
		TypeExpr::Generic(name, args) => {
			TypeExpr::Generic(name.clone(), args.iter().map(|a| replace_self(a, self_ty)).collect())
		}
		other => other.clone(),
	}
}

#[derive(Clone)]
pub(crate) struct Local {
	pub var: Variable,
	pub typ: Typ,
	pub mutable: bool,
	pub boxed: bool,
}

impl Local {
	pub fn plain(var: Variable, typ: Typ, mutable: bool) -> Self {
		Local {
			var,
			typ,
			mutable,
			boxed: false,
		}
	}
}

// `continue` jumps to `top`, `break` jumps to `exit`
pub(crate) struct LoopFrame {
	pub top: Block,
	pub exit: Option<Block>,
	pub depth: usize,
}

pub struct Compiler {
	builder_ctx: FunctionBuilderContext,
	ctx: codegen::Context,
	module: JITModule,
	string_idx: usize,
	atoms: HashSet<String>,
	generics: HashMap<String, GenericFnDef>,
	mono: HashMap<String, FnSig>,
	pending: Vec<Pending>,
	trait_impls: HashSet<(String, String)>,
	descs: HashMap<String, DataId>,
	publics: HashSet<String>,
	reexports: HashMap<String, String>,
	consts: HashMap<String, Spanned<Expr>>,
}

impl Default for Compiler {
	fn default() -> Self {
		let mut flag_builder = settings::builder();
		flag_builder.set("use_colocated_libcalls", "false").unwrap();
		flag_builder.set("is_pic", "false").unwrap();
		let isa = cranelift_native::builder()
			.unwrap_or_else(|e| panic!("unsupported host: {e}"))
			.finish(settings::Flags::new(flag_builder))
			.unwrap();
		let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
		builder.symbol(runtime::STR_CONCAT, runtime::str_concat as *const u8);
		builder.symbol(runtime::STR_MARK, runtime::str_mark as *const u8);
		builder.symbol(runtime::STR_TAKE, runtime::str_take as *const u8);
		builder.symbol(runtime::ALLOC, runtime::alloc as *const u8);
		builder.symbol(runtime::ARRAY_SHARE, runtime::array_share as *const u8);
		builder.symbol(runtime::ARRAY_COW, runtime::array_cow as *const u8);
		builder.symbol(runtime::ARRAY_RELEASE, runtime::array_release as *const u8);
		builder.symbol(runtime::MAP_RELEASE, runtime::map_release as *const u8);
		builder.symbol(runtime::WRITE, runtime::write as *const u8);
		builder.symbol(runtime::WRITE_SEP, runtime::write_sep as *const u8);
		builder.symbol(runtime::SLICE, runtime::slice as *const u8);
		builder.symbol(runtime::ARRAY_WRITE_BACK, runtime::array_write_back as *const u8);
		builder.symbol(runtime::PANIC_OOB, runtime::panic_oob as *const u8);
		builder.symbol(runtime::ARRAY_RESERVE, runtime::array_reserve as *const u8);
		builder.symbol(runtime::ARRAY_EXTEND, runtime::array_extend as *const u8);
		builder.symbol(runtime::STR_EQ, runtime::str_eq as *const u8);
		builder.symbol(runtime::STR_CONTAINS, runtime::str_contains as *const u8);
		builder.symbol(runtime::ASSERT_FAIL, runtime::assert_fail as *const u8);
		builder.symbol(runtime::PANIC, runtime::panic as *const u8);
		builder.symbol(runtime::MAP_NEW, runtime::map_new as *const u8);
		builder.symbol(runtime::MAP_GET, runtime::map_get as *const u8);
		builder.symbol(runtime::MAP_SET, runtime::map_set as *const u8);
		builder.symbol(runtime::MAP_DELETE, runtime::map_delete as *const u8);
		builder.symbol(runtime::MAP_SHARE, runtime::map_share as *const u8);
		builder.symbol(runtime::REF_SHARE, runtime::ref_share as *const u8);
		builder.symbol(runtime::REF_RELEASE, runtime::ref_release as *const u8);

		let module = JITModule::new(builder);
		Self {
			builder_ctx: FunctionBuilderContext::new(),
			ctx: module.make_context(),
			module,
			string_idx: 0,
			atoms: HashSet::new(),
			generics: HashMap::new(),
			mono: HashMap::new(),
			pending: Vec::new(),
			trait_impls: HashSet::new(),
			descs: HashMap::new(),
			publics: HashSet::new(),
			reexports: HashMap::new(),
			consts: HashMap::new(),
		}
	}
}

impl Compiler {
	// Register a type's fills as `Type.method` fns.
	fn register_fills<'a>(
		&mut self,
		typ: &str,
		type_params: &[TypeParam],
		fills: &'a [Spanned<Expr>],
		scope: &'a Scope,
		others: &mut Vec<FnItem<'a>>,
	) -> Result<(), Diagnostic> {
		for m in fills {
			let Expr::Fn {
				name,
				type_params: mtp,
				params,
				params_tuple,
				ret,
				body,
			} = &m.0
			else {
				continue;
			};
			// one fill per name
			let key = format!("{typ}.{name}");
			if others.iter().any(|f| f.key == key) || self.generics.contains_key(&key) {
				let msg = format!("duplicate fill `{key}`");
				return Err(Diagnostic::new(msg, m.1.into_range()).with_label("one fill per name"));
			}
			if type_params.is_empty() && mtp.is_empty() {
				others.push(FnItem {
					key: format!("{typ}.{name}"),
					scope,
					params,
					params_tuple: *params_tuple,
					ret,
					body,
				});
				continue;
			}
			let self_ty = if type_params.is_empty() {
				TypeExpr::Name(typ.to_string())
			} else {
				let args = type_params.iter().map(|p| TypeExpr::Name(p.name.clone())).collect();
				TypeExpr::Generic(typ.to_string(), args)
			};
			let params = params
				.iter()
				.map(|p| Param {
					typ: replace_self(&p.typ, &self_ty),
					..p.clone()
				})
				.collect();
			let ret = ret.as_ref().map(|(te, span)| (replace_self(te, &self_ty), *span));
			let mut all_params = type_params.to_vec();
			all_params.extend(mtp.clone());
			self.generics.insert(
				key,
				GenericFnDef {
					params,
					params_tuple: *params_tuple,
					ret,
					body: body.clone(),
					type_params: all_params,
					captures: vec![],
					self_name: None,
				},
			);
		}
		Ok(())
	}

	pub fn compile(&mut self, program: &Program) -> Result<*const u8, Diagnostic> {
		let mut struct_items: Vec<(&str, &[Param])> = vec![];
		let mut generics = Generics::default();
		let mut enum_items: Vec<EnumItem> = vec![];
		let mut alias_items: Vec<(&str, &TypeExpr)> = vec![];
		let mut soft_aliases: Vec<(String, TypeExpr)> = vec![];
		let mut main_body: Option<&[Spanned<Expr>]> = None;
		let mut others: Vec<FnItem> = vec![];
		let mut loose_refs: Vec<&Spanned<Expr>> = vec![];
		let mut trait_bodies: Vec<TraitBody> = vec![];

		self.publics = program.publics.clone();
		self.reexports = program.reexports.clone();
		self.consts = program.consts.clone();
		let scopes: HashMap<&str, &Scope> = program.modules.iter().map(|m| (m.name.as_str(), &m.scope)).collect();
		let scope_of = |key: &str| scopes[key.split_once("::").map_or("main", |(m, _)| m)];

		let mut traits: HashMap<&str, TraitItem> = HashMap::new();
		for (_, item) in program.items() {
			let (e, span) = item;
			let Expr::TraitDef {
				name,
				supers,
				fields,
				methods,
			} = e
			else {
				continue;
			};
			let item = (supers.as_slice(), fields.as_slice(), methods.as_slice());
			if traits.insert(name.as_str(), item).is_some() {
				let msg = format!("duplicate trait `{name}`");
				return Err(Diagnostic::new(msg, span.into_range()).with_label("already defined"));
			}
		}
		for (scope, item) in program.items() {
			match &item.0 {
				Expr::StructDef {
					name,
					type_params,
					fields,
					fills,
				} if !type_params.is_empty() => {
					generics.structs.insert(
						name.clone(),
						GenericStructDef {
							type_params: type_params.clone(),
							fields: fields.clone(),
						},
					);
					self.register_fills(name, type_params, fills, scope, &mut others)?;
				}
				Expr::StructDef {
					name, fields, fills, ..
				} => {
					struct_items.push((name.as_str(), fields.as_slice()));
					self.register_fills(name, &[], fills, scope, &mut others)?;
				}
				Expr::EnumDef {
					name,
					type_params,
					variants,
					fills,
					..
				} if !type_params.is_empty() => {
					generics.enums.insert(
						name.clone(),
						GenericEnumDef {
							type_params: type_params.clone(),
							variants: variants.clone(),
						},
					);
					self.register_fills(name, type_params, fills, scope, &mut others)?;
				}
				Expr::EnumDef {
					name,
					backing,
					variants,
					fills,
					..
				} => {
					enum_items.push((name.as_str(), backing.as_ref(), variants.as_slice()));
					self.register_fills(name, &[], fills, scope, &mut others)?;
				}
				Expr::TypeAlias { name, typ } => {
					if matches!(typ, TypeExpr::TupleStruct(..)) && TypeCtx::builtin_type(name) {
						let msg = format!("`{name}` is a builtin type");
						return Err(Diagnostic::new(msg, item.1.into_range()).with_label("pick another struct name"));
					}
					alias_items.push((name.as_str(), typ));
				}
				Expr::TraitDef { .. } => {}
				Expr::Claim {
					typ,
					type_params,
					traits: ts,
					fills,
				} if fills.is_empty()
					&& type_params.is_empty()
					&& !typ.contains("::")
					&& matches!(ts.as_slice(), [t] if !traits.contains_key(t.as_str())) =>
				{
					loose_refs.push(item)
				}
				Expr::Claim {
					typ,
					type_params,
					traits: claimed,
					fills,
				} => {
					for tn in claimed {
						if !type_params.is_empty() {
							let msg = "generic trait claims aren't supported yet".to_string();
							return Err(
								Diagnostic::new(msg, item.1.into_range()).with_label("remove the type parameters")
							);
						}
						trait_bodies.push(TraitBody {
							span: item.1,
							typ,
							trait_name: tn,
							methods: fills,
							scope,
						});
						self.trait_impls.insert((typ.clone(), tn.clone()));
					}
					self.register_fills(typ, type_params, fills, scope, &mut others)?;
				}
				Expr::Fn { name, body, .. } if name == "main" => main_body = Some(body),
				Expr::Fn {
					name,
					type_params,
					params,
					params_tuple,
					ret,
					body,
				} if !type_params.is_empty() => {
					self.generics.insert(
						name.clone(),
						GenericFnDef {
							params: params.clone(),
							params_tuple: *params_tuple,
							ret: ret.clone(),
							body: body.clone(),
							type_params: type_params.clone(),
							captures: vec![],
							self_name: None,
						},
					);
				}
				Expr::Fn {
					name,
					params,
					params_tuple,
					ret,
					body,
					..
				} => others.push(FnItem {
					key: name.clone(),
					scope,
					params,
					params_tuple: *params_tuple,
					ret,
					body,
				}),
				Expr::Doc(_) => {}
				Expr::Bind {
					mutable: false,
					name,
					typ: None,
					value: Some(v),
				} => {
					if let Some(te) = TypeExpr::from_expr(&v.0) {
						soft_aliases.push((name.clone(), te));
					}
					loose_refs.push(item);
				}
				_ => loose_refs.push(item),
			}
		}

		let mut aliases: HashMap<String, TypeExpr> =
			alias_items.iter().map(|(name, te)| (name.to_string(), (*te).clone())).collect();
		aliases.extend(soft_aliases);

		// name-only registry
		let enum_names: HashMap<String, Vec<VariantInfo>> =
			enum_items.iter().map(|(name, ..)| (name.to_string(), Vec::new())).collect();

		// structs can ref each other
		let no_type_params: HashMap<String, Typ> = HashMap::new();
		let mut structs: HashMap<String, Vec<FieldDef>> = HashMap::new();
		let mut pending = struct_items;
		let mut placeholders: HashSet<String> = HashSet::new();
		let field = |types: &TypeCtx, p: &Param| {
			let typ = types.resolve(&p.typ, p.span)?;
			if matches!(typ, Typ::Ref(_)) && p.default.is_none() {
				let msg = "a reference field must be optional (`?&T`) or have a default";
				return Err(Diagnostic::new(msg, p.span.into_range()).with_label("no zero value for `&T`"));
			}
			Ok(FieldDef {
				name: p.name.clone(),
				typ,
				default: p.default.clone(),
			})
		};
		while !pending.is_empty() {
			let (mut done, mut err) = (vec![], None);
			pending.retain(|(name, fields)| {
				let types = TypeCtx::new(&structs, &enum_names, &aliases, &no_type_params, &generics, &traits)
					.with_scope(scope_of(name));
				let resolve = || {
					let fs: Vec<FieldDef> = fields.iter().map(|p| field(&types, p)).collect::<Result<_, _>>()?;
					for (p, f) in fields.iter().zip(&fs) {
						if !ref_guarded(&f.typ, &placeholders) {
							return Err(Diagnostic::new(
								format!("`{name}` recurses for ever ever"),
								p.span.into_range(),
							)
							.with_label("would require infinitely nested fields"));
						}
					}
					Ok(fs)
				};
				match resolve() {
					Ok(fs) => {
						done.push((name.to_string(), fs));
						false
					}
					Err(e) => {
						err = Some(e);
						true
					}
				}
			});
			if done.is_empty() {
				if !placeholders.is_empty() {
					return Err(err.unwrap());
				}
				placeholders.extend(pending.iter().map(|(n, _)| n.to_string()));
				structs.extend(placeholders.iter().map(|n| (n.clone(), vec![])));
			}
			structs.extend(done);
		}
		let field_types = TypeCtx::new(&structs, &enum_names, &aliases, &no_type_params, &generics, &traits);
		check_impls(trait_bodies, &traits, &self.trait_impls, field_types, &mut others)?;

		let enums: HashMap<String, Vec<VariantInfo>> = enum_items
			.iter()
			.map(|(name, backing, variants)| {
				let types = TypeCtx::new(&structs, &enum_names, &aliases, &no_type_params, &generics, &traits)
					.with_scope(scope_of(name));
				let mut vs = build_variants(variants, types)?;
				if let Some(bt) = backing {
					apply_backing(bt, &mut vs, variants, types)?;
				}
				Ok((name.to_string(), vs))
			})
			.collect::<Result<_, _>>()?;

		// hoist fns
		let mut funcs: HashMap<String, FnSig> = HashMap::new();
		for item in &others {
			let mut aliases = aliases.clone();
			if let Some(t) = item.key.rsplit_once('.').map(|(t, _)| t) {
				aliases.insert("Self".into(), TypeExpr::Name(t.into()));
			}
			let types =
				TypeCtx::new(&structs, &enums, &aliases, &no_type_params, &generics, &traits).with_scope(item.scope);
			let params: Vec<Typ> = item
				.params
				.iter()
				.map(|p| types.resolve(&p.typ, p.span))
				.collect::<Result<_, _>>()?;
			let muts: Vec<bool> = item.params.iter().map(|p| p.mutable).collect();
			let ret = match item.ret {
				Some((ret_te, ret_span)) => types.resolve(ret_te, *ret_span)?,
				None => Typ::unit(),
			};
			let sig = self.declare_fn(&item.key, params, muts, ret);
			funcs.insert(item.key.clone(), sig);
		}

		for item in &others {
			let self_type = item.key.rsplit_once('.').map(|(t, _)| t);
			let mut aliases = aliases.clone();
			if let Some(t) = self_type {
				aliases.insert("Self".into(), TypeExpr::Name(t.into()));
			}
			let types =
				TypeCtx::new(&structs, &enums, &aliases, &no_type_params, &generics, &traits).with_scope(item.scope);
			let (params, ret) = types.resolve_params_ret(item.params, item.ret)?;
			let ret = ret.or_else(|| Some((funcs[&item.key].ret.clone(), (0..0).into())));
			self.translate(
				FnDef {
					params: &params,
					params_tuple: item.params_tuple,
					ret,
					body: item.body,
					self_type,
					..FnDef::default()
				},
				&funcs,
				types,
			)?;
			self.finish_fn(&oi_symbol(&item.key));
		}

		// a `str` wrapper per struct
		let mut render = HashMap::new();
		for (name, _) in self.trait_impls.clone() {
			if render.contains_key(&name) {
				continue;
			}
			let types = TypeCtx::new(&structs, &enums, &aliases, &no_type_params, &generics, &traits);
			let styp = types.named(&name, (0..0).into())?;
			let param = [("self".into(), styp.clone(), false)];
			let def = FnDef {
				params: &param,
				..FnDef::default()
			};
			let (mut trans, block) = self.translator(&def, &funcs, types);
			let val = trans.b.block_params(block)[0];
			let s = trans.derived_str(val, &styp);
			trans.emit_return(s, Typ::Str, (0..0).into())?;
			trans.b.finalize();
			render.insert(name.clone(), self.finish_fn(&oi_symbol(&format!("{name}#str"))));
		}

		// define vtables now that every concrete method has a FuncId
		for (typ, tn) in &self.trait_impls {
			if tn == "Drop" {
				continue;
			}
			let (_, tfields, tmethods) = traits[tn.as_str()];
			let methods: Vec<&str> = trait_fns(tmethods).map(|(n, ..)| n).collect();
			let m = methods.len();
			let f = tfields.len();
			let mut bytes = vec![0u8; (m + f + 1) * 8];
			for (i, tf) in tfields.iter().enumerate() {
				let idx = structs[typ.as_str()]
					.iter()
					.position(|fd| fd.name == tf.name)
					.expect("field checked by trait impl");
				bytes[(m + i) * 8..(m + i + 1) * 8].copy_from_slice(&((idx * 8) as i64).to_le_bytes());
			}
			let mut desc = DataDescription::new();
			desc.define(bytes.into_boxed_slice());
			for (i, name) in methods.iter().enumerate() {
				let id = funcs[&format!("{typ}.{name}")].id;
				let fref = self.module.declare_func_in_data(id, &mut desc);
				desc.write_function_addr((i * 8) as u32, fref);
			}
			let fref = self.module.declare_func_in_data(render[typ.as_str()], &mut desc);
			desc.write_function_addr(((m + f) * 8) as u32, fref);
			let sym = oi_symbol(&format!("vtable_{typ}_{tn}"));
			let id = self
				.module
				.declare_data(&sym, Linkage::Local, false, false)
				.expect("declare vtable");
			self.module.define_data(id, &desc).expect("define vtable");
		}

		// gather loose top-level statements
		let loose: Vec<Spanned<Expr>>;
		let entry: &[Spanned<Expr>] = match main_body {
			Some(body) => {
				if let Some(first) = loose_refs.first() {
					return Err(Diagnostic::new(
						"top-level statements are not allowed alongside `fn main`",
						first.1.into_range(),
					)
					.with_label("move this inside a function")
					.with_note("`fn main` is the entrypoint, so loose statements have nowhere to run"));
				}
				body
			}
			None => {
				loose = loose_refs.into_iter().cloned().collect();
				&loose
			}
		};

		let types =
			TypeCtx::new(&structs, &enums, &aliases, &no_type_params, &generics, &traits).with_scope(scopes["main"]);
		let typ = self.translate(
			FnDef {
				params_tuple: true,
				body: entry,
				is_main: true,
				..FnDef::default()
			},
			&funcs,
			types,
		)?;
		let entry_id = self.finish_fn("oi_main");

		// drain generic instances queued by calls we've seen
		while let Some((sym, def, subst)) = self.pending.pop() {
			let types = TypeCtx::new(&structs, &enums, &aliases, &subst, &generics, &traits);
			let (params, ret) = types.resolve_params_ret(&def.params, &def.ret)?;
			let ret = ret.or_else(|| Some((self.mono[&sym].ret.clone(), (0..0).into())));
			let self_sig = self.mono[&sym].clone();
			self.translate(
				FnDef {
					params: &params,
					params_tuple: def.params_tuple,
					ret,
					body: &def.body,
					captures: &def.captures,
					self_fn: def.self_name.as_deref().map(|n| (n, &self_sig)),
					..FnDef::default()
				},
				&funcs,
				types,
			)?;
			self.finish_fn(&sym);
		}

		let id = self.compile_entry(entry_id, typ, &funcs, types);

		self.module.finalize_definitions().expect("finalize definitions");
		Ok(self.module.get_finalized_function(id))
	}

	fn compile_entry(&mut self, entry: FuncId, typ: Typ, funcs: &HashMap<String, FnSig>, types: TypeCtx) -> FuncId {
		let (mut trans, _) = self.translator(&FnDef::default(), funcs, types);

		let callee = trans.module.declare_func_in_func(entry, trans.b.func);
		let call = trans.b.ins().call(callee, &[]);
		if let Some(val) = trans.b.inst_results(call).first().copied() {
			trans.emit_print(val, &typ, false, runtime::Sink::Out);
			trans.write_lit("\n", runtime::Sink::Out);
		}
		trans.b.ins().return_(&[]);
		trans.b.finalize();

		self.finish_fn("__oi_main")
	}

	// Declare a hoisted fn's signature ahead of its body.
	fn declare_fn(&mut self, key: &str, params: Vec<Typ>, muts: Vec<bool>, ret: Typ) -> FnSig {
		let int = self.module.target_config().pointer_type();
		let mut sig = self.module.make_signature();
		sig.params.extend(params.iter().map(|t| AbiParam::new(cl_type(t, int))));
		if !ret.is_unit() {
			sig.returns.push(AbiParam::new(cl_type(&ret, int)));
		}
		let id = self
			.module
			.declare_function(&oi_symbol(key), Linkage::Local, &sig)
			.expect("declare function");
		FnSig { id, params, muts, ret }
	}

	fn finish_fn(&mut self, name: &str) -> FuncId {
		let id = self
			.module
			.declare_function(name, Linkage::Local, &self.ctx.func.signature)
			.expect("declare function");
		self.module.define_function(id, &mut self.ctx).expect("define function");
		self.module.clear_context(&mut self.ctx);
		id
	}

	fn translator<'a>(
		&'a mut self,
		def: &FnDef,
		funcs: &'a HashMap<String, FnSig>,
		types: TypeCtx<'a>,
	) -> (Translator<'a>, Block) {
		let int = self.module.target_config().pointer_type();
		let mut b = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_ctx);
		for (_, typ, _) in def.params {
			b.func.signature.params.push(AbiParam::new(cl_type(typ, int)));
		}
		if !def.captures.is_empty() {
			b.func.signature.params.push(AbiParam::new(int));
		}
		let block = b.create_block();
		b.append_block_params_for_function_params(block);
		b.switch_to_block(block);
		b.seal_block(block);

		let trans = Translator {
			int,
			b,
			vars: HashMap::new(),
			params: vec![],
			dollar: None,
			module: &mut self.module,
			funcs,
			structs: types.structs,
			enums: types.enums,
			aliases: types.aliases,
			type_params: types.type_params,
			generics: types.generics,
			traits: types.traits,
			generic_fns: &self.generics,
			trait_impls: &self.trait_impls,
			scope: types.scope,
			publics: &self.publics,
			reexports: &self.reexports,
			consts: &self.consts,
			mono: &mut self.mono,
			pending: &mut self.pending,
			descs: &mut self.descs,
			string_idx: &mut self.string_idx,
			atoms: &mut self.atoms,
			ret: def.ret.clone(),
			loops: vec![],
			scopes: vec![vec![]],
			self_type: def.self_type.map(str::to_owned),
			is_main: def.is_main,
			self_name: None,
		};

		(trans, block)
	}

	fn translate(&mut self, def: FnDef, funcs: &HashMap<String, FnSig>, types: TypeCtx) -> Result<Typ, Diagnostic> {
		let decl_span = def.ret.as_ref().map(|(_, s)| *s);
		let (mut trans, block) = self.translator(&def, funcs, types);

		let param_vals: Vec<Value> = trans.b.block_params(block).to_vec();
		for ((name, typ, mutable), &val) in def.params.iter().zip(param_vals.iter()) {
			let cl = trans.b.func.dfg.value_type(val);
			let var = trans.b.declare_var(cl);
			trans.b.def_var(var, val);
			let local = Local {
				var,
				typ: typ.clone(),
				mutable: *mutable,
				boxed: *mutable && name != "self",
			};
			trans.vars.insert(name.clone(), local.clone());
			trans.params.push(local);
		}
		trans.bind_dollar(def.params_tuple);

		if !def.captures.is_empty() {
			let env = param_vals[def.params.len()];
			for (i, (name, typ, boxed)) in def.captures.iter().enumerate() {
				let cl = if *boxed { trans.int } else { cl_type(typ, trans.int) };
				let val = trans.b.ins().load(cl, MemFlags::new(), env, ((i + 1) * 8) as i32);
				let var = trans.b.declare_var(cl);
				trans.b.def_var(var, val);
				let local = Local {
					var,
					typ: typ.clone(),
					mutable: *boxed,
					boxed: *boxed,
				};
				trans.vars.insert(name.clone(), local);
			}
		}

		// fn literal is bound in its own body and so it can refer to itself
		if let Some((name, sig)) = def.self_fn {
			let func_ref = trans.module.declare_func_in_func(sig.id, trans.b.func);
			let addr = trans.b.ins().func_addr(trans.int, func_ref);
			let typ = Typ::Fn(sig.value_params(), Box::new(sig.ret.clone()));
			trans.bind_local(name, addr, typ, false);
		}

		let tail_target = trans.ret.as_ref().map(|(t, _)| t.clone());
		if let Some((val, typ)) = trans.block_tail(def.body, tail_target.as_ref())? {
			let span = def.body.last().map(|s| s.1).or(decl_span).unwrap_or((0..0).into());
			if let Some(e) = def.body.last() {
				trans.move_resource(e, &typ)?;
			}
			trans.emit_return(val, typ, span)?;
		}
		trans.b.finalize();

		Ok(trans.ret.map(|(t, _)| t).unwrap_or(Typ::unit()))
	}
}
