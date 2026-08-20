//! The Oi type layer.

use std::fmt;

use cranelift::prelude::*;

use crate::ast::{Expr, Param, Spanned, TypeExpr};

#[derive(Clone, Debug)]
pub(crate) enum Typ {
	Int(u16),
	UInt(u16),
	ISize,
	USize,
	Float(u16),
	Bool,
	Str,
	Atom,
	Tuple(Vec<(Option<String>, Typ)>),
	Array(Box<Typ>),
	FixedArray(Box<Typ>, usize),
	Struct(String, Vec<FieldDef>),
	TupleStruct(String, Vec<(Option<String>, Typ)>),
	Enum(String),
	Trait(String),
	Option(Box<Typ>),
	Result(Box<Typ>),
	Sum(Vec<VariantInfo>),
	Error,
	Range,
	Fn(Vec<Typ>, Box<Typ>),
	Closure(Vec<Typ>, Box<Typ>, bool),
	Map(Box<Typ>, Box<Typ>),
	Mut(Box<Typ>),
	Ref(Box<Typ>),
}

// A struct field definition.
#[derive(Clone, Debug)]
pub(crate) struct FieldDef {
	pub name: String,
	pub typ: Typ,
	pub default: Option<Spanned<Expr>>,
	pub embedded: bool,
}

// Check whether a field is embedded.
pub(crate) fn embedded(p: &Param) -> bool {
	matches!(&p.typ, TypeExpr::Name(n) if *n == p.name)
}

// The embedded fields of a struct.
pub(crate) fn embeds(fields: &[FieldDef]) -> impl Iterator<Item = (usize, &str, &[FieldDef])> {
	fields.iter().enumerate().filter_map(|(o, f)| match &f.typ {
		Typ::Struct(sn, inner) if f.embedded => Some((o, sn.as_str(), &inner[..])),
		_ => None,
	})
}

// A field's encoded vtable slot.
pub(crate) fn field_slot<'f>(fields: &'f [FieldDef], name: &str) -> Option<(i64, &'f FieldDef)> {
	let direct = fields.iter().position(|f| f.name == name).map(|i| ((i * 8) as i64, &fields[i]));
	direct.or_else(|| {
		embeds(fields).find_map(|(o, _, inner)| {
			let i = inner.iter().position(|f| f.name == name)?;
			Some((((i * 8) as i64) << 32 | ((o * 8) as i64) | 1, &inner[i]))
		})
	})
}

impl Typ {
	pub fn unit() -> Typ {
		Typ::Tuple(vec![])
	}

	pub fn is_unit(&self) -> bool {
		matches!(self, Typ::Tuple(f) if f.is_empty())
	}

	pub fn is_enumish(&self) -> bool {
		matches!(self, Typ::Enum(_) | Typ::Option(_) | Typ::Result(_) | Typ::Sum(..))
	}

	// 1:1 spelling for identity keys.
	pub fn key(&self) -> String {
		let keys = |ts: &[Typ]| ts.iter().map(Typ::key).collect::<Vec<_>>().join(", ");
		match self {
			Typ::Tuple(fields) => {
				let elems: Vec<_> = fields.iter().map(|(_, t)| t.key()).collect();
				format!("({})", elems.join(", "))
			}
			Typ::Array(e) => format!("[]{}", e.key()),
			Typ::FixedArray(e, n) => format!("[{n}]{}", e.key()),
			Typ::Option(inner) => format!("?{}", inner.key()),
			Typ::Result(inner) => format!("!{}", inner.key()),
			Typ::Map(k, v) => format!("Map[{}, {}]", k.key(), v.key()),
			Typ::Mut(inner) => format!("mut {}", inner.key()),
			Typ::Ref(inner) => format!("&{}", inner.key()),
			Typ::Trait(name) => format!("dyn {name}"),
			Typ::Fn(params, ret) => format!("fn({}) {}", keys(params), ret.key()),
			Typ::Closure(params, ret, _) => format!("closure({}) {}", keys(params), ret.key()),
			Typ::Sum(variants) => variants
				.iter()
				.map(|v| match v.payload.is_empty() {
					true => format!(":{}", v.name),
					false => format!("{}({})", v.name, keys(&v.payload)),
				})
				.collect::<Vec<_>>()
				.join(" | "),
			_ => self.to_string(),
		}
	}
}

impl fmt::Display for Typ {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Typ::Int(32) => write!(f, "int"),
			Typ::Int(w) => write!(f, "i{w}"),
			Typ::UInt(w) => write!(f, "u{w}"),
			Typ::ISize => write!(f, "isize"),
			Typ::USize => write!(f, "usize"),
			Typ::Float(64) => write!(f, "float"),
			Typ::Float(w) => write!(f, "f{w}"),
			Typ::Bool => write!(f, "bool"),
			Typ::Str => write!(f, "str"),
			Typ::Atom => write!(f, "atom"),
			Typ::Tuple(fields) if fields.is_empty() => write!(f, "()"),
			Typ::Tuple(_) => write!(f, "tuple"),
			Typ::Array(e) => write!(f, "[]{e}"),
			Typ::FixedArray(e, n) => write!(f, "[{n}]{e}"),
			Typ::Struct(name, _) => write!(f, "{name}"),
			Typ::TupleStruct(name, _) => write!(f, "{name}"),
			Typ::Enum(name) => write!(f, "{name}"),
			Typ::Trait(name) => write!(f, "{name}"),
			Typ::Option(inner) => write!(f, "?{inner}"),
			Typ::Result(inner) => write!(f, "!{inner}"),
			Typ::Sum(variants) => {
				write!(
					f,
					"{}",
					variants
						.iter()
						.map(|v| if v.payload.is_empty() {
							format!(":{}", v.name)
						} else {
							v.name.clone()
						})
						.collect::<Vec<_>>()
						.join(" | ")
				)
			}
			Typ::Error => write!(f, "Error"),
			Typ::Range => write!(f, "range"),
			Typ::Fn(params, ret) | Typ::Closure(params, ret, _) => {
				write!(f, "fn(")?;
				for (i, p) in params.iter().enumerate() {
					if i > 0 {
						write!(f, ", ")?;
					}
					write!(f, "{p}")?;
				}
				write!(f, ") {ret}")
			}
			Typ::Map(k, v) => write!(f, "Map[{k}, {v}]"),
			Typ::Mut(inner) => write!(f, "mut {inner}"),
			Typ::Ref(inner) => write!(f, "&{inner}"),
		}
	}
}

impl PartialEq for FieldDef {
	fn eq(&self, other: &Self) -> bool {
		self.name == other.name && self.typ == other.typ
	}
}

// Ignore tuple field names for equality.
impl PartialEq for Typ {
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(Typ::Tuple(a), Typ::Tuple(b)) => a.len() == b.len() && a.iter().zip(b).all(|((_, x), (_, y))| x == y),
			(Typ::TupleStruct(a, _), Typ::TupleStruct(b, _)) => a == b,
			(Typ::Int(a), Typ::Int(b)) | (Typ::UInt(a), Typ::UInt(b)) | (Typ::Float(a), Typ::Float(b)) => a == b,
			(Typ::Struct(n, a), Typ::Struct(m, b)) => n == m && a == b,
			(Typ::Enum(a), Typ::Enum(b)) => a == b,
			(Typ::Trait(a), Typ::Trait(b)) => a == b,
			(Typ::Option(a), Typ::Option(b)) | (Typ::Result(a), Typ::Result(b)) | (Typ::Array(a), Typ::Array(b)) => {
				a == b
			}
			(Typ::FixedArray(a, n), Typ::FixedArray(b, m)) => a == b && n == m,
			(Typ::Sum(a), Typ::Sum(b)) => a == b,
			(Typ::Fn(p, r), Typ::Fn(q, s)) | (Typ::Closure(p, r, _), Typ::Closure(q, s, _)) => p == q && r == s,
			(Typ::Map(k, v), Typ::Map(l, w)) => k == l && v == w,
			(Typ::Mut(a), Typ::Mut(b)) => a == b,
			(Typ::Ref(a), Typ::Ref(b)) => match (&**a, &**b) {
				(Typ::Struct(n, _), Typ::Struct(m, _)) => n == m,
				_ => a == b,
			},
			_ => std::mem::discriminant(self) == std::mem::discriminant(other),
		}
	}
}

// Spell a type back out as source, for signatures created by the compiler.
pub(crate) fn type_expr(typ: &Typ) -> Option<TypeExpr> {
	let named = |n: &String| (!n.contains('[')).then(|| TypeExpr::Name(n.clone()));
	Some(match typ {
		Typ::Int(_) | Typ::UInt(_) | Typ::ISize | Typ::USize | Typ::Float(_) | Typ::Bool | Typ::Str => {
			TypeExpr::Name(typ.to_string())
		}
		Typ::Range => TypeExpr::Name("range".into()),
		Typ::Struct(n, _) | Typ::TupleStruct(n, _) | Typ::Enum(n) => named(n)?,
		Typ::Array(e) => TypeExpr::Array(Box::new(type_expr(e)?)),
		Typ::FixedArray(e, n) => TypeExpr::FixedArray(Box::new(type_expr(e)?), *n),
		Typ::Option(e) => TypeExpr::Option(Box::new(type_expr(e)?)),
		Typ::Result(e) => TypeExpr::Result(Box::new(type_expr(e)?), None),
		Typ::Map(k, v) => TypeExpr::Map(Box::new(type_expr(k)?), Box::new(type_expr(v)?)),
		Typ::Tuple(fs) => TypeExpr::Tuple(fs.iter().map(|(_, t)| type_expr(t)).collect::<Option<_>>()?),
		Typ::Fn(ps, r) => TypeExpr::Fn(
			ps.iter().map(|p| type_expr(mut_peel(p))).collect::<Option<_>>()?,
			ps.iter().map(|p| matches!(p, Typ::Mut(_))).collect(),
			Box::new(type_expr(r)?),
		),
		_ => return None,
	})
}

fn mut_peel(typ: &Typ) -> &Typ {
	match typ {
		Typ::Mut(inner) => inner,
		typ => typ,
	}
}

pub(crate) fn oi_symbol(name: &str) -> String {
	format!("oi_{}", name.replace('.', "__").replace("::", "$"))
}

// Strip qualifiers like `module::`.
pub(crate) fn display_name(name: &str) -> &str {
	name.split_once("::").map_or(name, |(_, rest)| rest)
}

pub(crate) fn cl_int_for_width(w: u16) -> types::Type {
	match w {
		1..=8 => types::I8,
		9..=16 => types::I16,
		17..=32 => types::I32,
		_ => types::I64,
	}
}

pub(crate) fn cl_type(typ: &Typ, int: types::Type) -> types::Type {
	match typ {
		Typ::Int(w) | Typ::UInt(w) => cl_int_for_width(*w),
		Typ::ISize | Typ::USize => int,
		Typ::Float(w) => match w {
			16 => types::F16,
			32 => types::F32,
			64 => types::F64,
			128 => types::F128,
			w => panic!("unsupported float width f{w}"),
		},
		_ => int,
	}
}

pub(crate) fn elem_size(typ: &Typ) -> i64 {
	match typ {
		Typ::Int(w) | Typ::UInt(w) => cl_int_for_width(*w).bytes() as i64,
		Typ::Float(w) => (*w as i64) / 8,
		_ => 8,
	}
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct VariantInfo {
	pub name: String,
	pub disc: i64,
	pub raw: Option<String>,
	pub payload: Vec<Typ>,
	pub names: Vec<String>,
	pub backing: Option<Typ>,
}

impl VariantInfo {
	pub fn new(name: impl Into<String>, disc: i64, payload: Vec<Typ>) -> Self {
		VariantInfo {
			name: name.into(),
			disc,
			raw: None,
			payload,
			names: vec![],
			backing: None,
		}
	}
}

// An enum is a tagged union if any variant has fields.
pub(crate) fn enum_boxed(variants: &[VariantInfo]) -> bool {
	variants.iter().any(|v| !v.payload.is_empty())
}

// Tag permutation to coerce a sum across definitions with the same member set.
pub(crate) fn sum_remap(src: &[VariantInfo], dst: &[VariantInfo]) -> Option<Vec<(i64, i64)>> {
	if src.len() != dst.len() {
		return None;
	}
	src.iter()
		.map(|s| {
			dst.iter()
				.find(|d| d.name == s.name && d.payload == s.payload)
				.map(|d| (s.disc, d.disc))
		})
		.collect()
}

// Slot count of a boxed enum.
pub(crate) fn enum_slots(variants: &[VariantInfo]) -> usize {
	// the tag plus the widest variant's fields
	1 + variants.iter().map(|v| v.payload.len()).max().unwrap_or(0)
}

pub(crate) fn option_variants(inner: &Typ) -> Vec<VariantInfo> {
	vec![
		VariantInfo::new("none", 0, vec![]),
		VariantInfo::new("some", 1, vec![inner.clone()]),
	]
}

pub(crate) fn result_variants(inner: &Typ) -> Vec<VariantInfo> {
	vec![
		VariantInfo::new("ok", 0, vec![inner.clone()]),
		VariantInfo::new("err", 1, vec![Typ::Error]),
	]
}

// An atom sum type desugars to a bare enum.
pub(crate) fn atom_sum_variants(names: &[String]) -> Vec<VariantInfo> {
	names
		.iter()
		.enumerate()
		.map(|(disc, name)| VariantInfo::new(name.clone(), disc as i64, vec![]))
		.collect()
}
