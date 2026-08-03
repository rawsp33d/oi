use super::generic::unify;
use super::*;

impl<'a> Translator<'a> {
	pub(super) fn str_const(&mut self, s: &str) -> Value {
		let mut bytes = s.as_bytes().to_vec();
		bytes.push(0);
		let sym = format!("__str_{}", *self.string_idx);
		*self.string_idx += 1;
		self.define_data(&sym, bytes);
		self.data_addr(&sym)
	}

	// Intern an atom name to a pointer-sized symbol.
	pub(super) fn atom_const(&mut self, name: &str) -> Value {
		let sym = format!("__atom_{name}");
		if self.atoms.insert(name.to_string()) {
			let mut bytes = format!(":{name}").into_bytes();
			bytes.push(0);
			self.define_data(&sym, bytes);
		}
		self.data_addr(&sym)
	}

	// Define a data symbol holding raw bytes.
	fn define_data(&mut self, sym: &str, bytes: Vec<u8>) {
		let id = self.module.declare_data(sym, Linkage::Local, false, false).unwrap();
		let mut desc = DataDescription::new();
		desc.define(bytes.into_boxed_slice());
		self.module.define_data(id, &desc).unwrap();
	}

	// The address of a data symbol.
	fn data_addr(&mut self, sym: &str) -> Value {
		let id = self.module.declare_data(sym, Linkage::Local, false, false).unwrap();
		let gv = self.module.declare_data_in_func(id, self.b.func);
		self.b.ins().symbol_value(self.int, gv)
	}

	pub(super) fn zero(&mut self, typ: &Typ) -> Value {
		match typ {
			Typ::Float(16) => self.b.ins().f16const(Ieee16::with_bits(0)),
			Typ::Float(32) => self.b.ins().f32const(0.0),
			Typ::Float(64) => self.b.ins().f64const(0.0),
			Typ::Float(128) => {
				let c = self.b.func.dfg.constants.insert(Ieee128::with_bits(0).into());
				self.b.ins().f128const(c)
			}
			Typ::Float(w) => panic!("unsupported float width f{w}"),
			Typ::Str | Typ::Error => self.str_const(""),
			Typ::Atom => self.atom_const(""),
			Typ::Int(w) => self.b.ins().iconst(cl_type(&Typ::Int(*w), self.int), 0),
			Typ::UInt(w) => self.b.ins().iconst(cl_type(&Typ::UInt(*w), self.int), 0),
			Typ::Bool | Typ::ISize | Typ::USize => self.b.ins().iconst(self.int, 0),
			Typ::Fn(..) | Typ::Closure(..) | Typ::Trait(_) => self.b.ins().iconst(self.int, 0),
			// default to first variant, with zero'd payload fields
			Typ::Enum(_) | Typ::Option(_) | Typ::Result(_) | Typ::Sum(..) => {
				let variants = self.variants_of(typ);
				let v = variants.first().cloned();
				let disc = v.as_ref().map_or(0, |v| v.disc);
				let fields: Vec<Value> =
					v.map(|v| v.payload.iter().map(|t| self.zero(t)).collect()).unwrap_or_default();
				self.make_enum(&variants, disc, &fields)
			}
			_ if typ.is_unit() => self.b.ins().iconst(self.int, 0),
			Typ::Struct(_, fields) => {
				let fields = fields.clone();
				let ptr = self.stack_slot((fields.len() * 8) as u32);
				for (i, f) in fields.iter().enumerate() {
					let z = self.zero(&f.typ);
					self.b.ins().store(MemFlags::new(), z, ptr, (i * 8) as i32);
				}
				ptr
			}
			Typ::TupleStruct(_, fields) => self.zero(&Typ::Tuple(fields.clone())),
			Typ::Tuple(fields) => {
				let fields = fields.clone();
				let ptr = self.call_alloc(fields.len());
				for (i, (_, ftyp)) in fields.iter().enumerate() {
					let z = self.zero(ftyp);
					self.b.ins().store(MemFlags::new(), z, ptr, (i * 8) as i32);
				}
				ptr
			}
			Typ::Array(_) => {
				let zero = self.b.ins().iconst(self.int, 0);
				self.make_array(zero, zero)
			}
			Typ::FixedArray(elem, n) => {
				let elem = (**elem).clone();
				let stride = self.elem_stride(&elem);
				let ptr = self.stack_slot((*n as i64 * stride) as u32);
				for i in 0..*n {
					let z = self.zero(&elem);
					self.store_elem(ptr, (i as i64 * stride) as i32, &elem, z);
				}
				ptr
			}
			Typ::Range => {
				let ptr = self.call_alloc(2);
				let z = self.b.ins().iconst(self.int, 0);
				self.b.ins().store(MemFlags::new(), z, ptr, 0);
				self.b.ins().store(MemFlags::new(), z, ptr, 8);
				ptr
			}
			Typ::Map(..) => self.call_map_new(),
		}
	}

	// A numeric literal takes the binding's declared type directly.
	pub(super) fn coerce_lit(&mut self, value: &Spanned<Expr>, target: &Typ) -> Result<Option<Value>, Diagnostic> {
		let (neg, inner) = match &value.0 {
			Expr::Negative(e) => (true, &e.0),
			v => (false, v),
		};
		let oob = |n| {
			Diagnostic::new(format!("{n} is out of range for {target}"), value.1.into_range())
				.with_label(format!("doesn't fit in {target}"))
		};
		let v = match (inner, target) {
			(Expr::Int(n), Typ::Int(w)) => {
				let n = if neg { -*n } else { *n };
				if n < int_min(*w) || n > int_max(*w) {
					return Err(oob(n));
				}
				self.b.ins().iconst(cl_int_for_width(*w), n)
			}
			(Expr::Int(n), Typ::UInt(w)) => {
				let n = if neg { -*n } else { *n };
				if n < 0 || (*w < 64 && n > uint_max(*w)) {
					return Err(oob(n));
				}
				self.b.ins().iconst(cl_int_for_width(*w), n)
			}
			(Expr::Int(n), Typ::ISize) => self.b.ins().iconst(self.int, if neg { -*n } else { *n }),
			(Expr::Int(n), Typ::USize) => {
				let n = if neg { -*n } else { *n };
				if n < 0 {
					return Err(oob(n));
				}
				self.b.ins().iconst(self.int, n)
			}
			(Expr::Int(n), Typ::Float(w)) => self.float_lit((if neg { -*n } else { *n }) as f64, *w, value.1)?,
			(Expr::Float(x), Typ::Float(w)) => self.float_lit(if neg { -*x } else { *x }, *w, value.1)?,
			(Expr::Atom(name), Typ::Enum(typ)) => self.construct_variant(typ, name, &[], value.1)?.0,
			(Expr::EnumShorthand { variant, args }, Typ::Enum(typ)) => {
				self.construct_variant(typ, variant, args, value.1)?.0
			}
			(Expr::None, Typ::Option(inner)) => self.make_enum(&option_variants(inner), 0, &[]),
			(Expr::Atom(name), Typ::Sum(..)) => {
				let variants = self.variants_of(target);
				let Some(v) = variants.iter().find(|v| &v.name == name && v.payload.is_empty()) else {
					return Err(
						Diagnostic::new(format!("`{target}` has no atom `:{name}`"), value.1.into_range())
							.with_label("not a member of this sum type"),
					);
				};
				self.make_enum(&variants, v.disc, &[])
			}
			_ => return Ok(None),
		};
		Ok(Some(v))
	}

	// The variant table of a named enum.
	pub(super) fn enum_variants(&self, name: &str) -> Vec<VariantInfo> {
		self.enums
			.get(name)
			.cloned()
			.or_else(|| self.generics.instances.borrow().get(name).cloned())
			.unwrap_or_default()
	}

	// Variant table for any type that carries variants.
	pub(super) fn variants_of(&self, typ: &Typ) -> Vec<VariantInfo> {
		match typ {
			Typ::Enum(name) => self.enum_variants(name),
			Typ::Option(inner) => option_variants(inner),
			Typ::Result(inner) => result_variants(inner),
			Typ::Sum(variants) => variants.clone(),
			_ => Vec::new(),
		}
	}

	// The tag of an enum value.
	pub(super) fn enum_tag(&mut self, variants: &[VariantInfo], val: Value) -> Value {
		if enum_boxed(variants) {
			self.b.ins().load(self.int, MemFlags::new(), val, 0)
		} else {
			val
		}
	}

	// Build a variant value.
	// A bare discriminant for fieldless enums, and a heap where that's not possible.
	pub(super) fn make_enum(&mut self, variants: &[VariantInfo], disc: i64, fields: &[Value]) -> Value {
		let slots = enum_slots(variants);
		if slots == 1 {
			return self.b.ins().iconst(self.int, disc);
		}
		let ptr = self.call_alloc(slots);
		let tag = self.b.ins().iconst(self.int, disc);
		self.b.ins().store(MemFlags::new(), tag, ptr, 0);
		for (i, fv) in fields.iter().enumerate() {
			self.b.ins().store(MemFlags::new(), *fv, ptr, ((i + 1) * 8) as i32);
		}
		ptr
	}

	// A match pattern's discriminant and payload binds.
	pub(super) fn enum_pattern(&self, pat: &Spanned<Expr>, typ: &Typ) -> Result<(i64, Vec<Bind>), Diagnostic> {
		let bad = |msg| Err(Diagnostic::new(msg, pat.1.into_range()).with_label("bad pattern"));
		if let (Typ::Sum(variants), Expr::Ident(v)) = (typ, &pat.0) {
			let disp = self.sum_display(v, pat.1);
			return match variants.iter().find(|x| x.name == disp) {
				Some(x) => Ok((x.disc, vec![])),
				None => bad(format!("`{typ}` has no variant `{v}`")),
			};
		}
		let (variant, args): (&str, &[Spanned<Expr>]) = match &pat.0 {
			Expr::EnumShorthand { variant, args } => (variant, args),
			Expr::Atom(v) => (v, &[]),
			Expr::Field { tuple, field } if matches!(tuple.0, Expr::Ident(_)) => (field, &[]),
			_ => return bad(format!("`{typ}` is matched by its variants")),
		};
		let variants = self.variants_of(typ);
		let Some(v) = variants.iter().find(|v| v.name == variant) else {
			return bad(format!("`{typ}` has no variant `{variant}`"));
		};
		let binds = if let [(Expr::Record(entries), _)] = args {
			let mut binds = Vec::with_capacity(entries.len());
			for (k, val) in entries {
				let (Expr::Ident(field), Expr::Ident(local)) = (&k.0, &val.0) else {
					return bad("field patterns bind names".into());
				};
				let Some(idx) = v.names.iter().position(|n| n == field) else {
					return bad(format!("`{variant}` has no field `{field}`"));
				};
				binds.push((local.clone(), v.payload[idx].clone(), 8 + idx as i32 * 8));
			}
			binds
		} else {
			field_binds(args.iter().zip(&v.payload), 8, 8)?
		};
		Ok((v.disc, binds))
	}

	// The display name a bare type-name pattern refers to.
	// ex: `string` -> `str`.
	fn sum_display(&self, name: &str, span: Span) -> String {
		self.types()
			.resolve(&TypeExpr::Name(name.to_string()), span)
			.map(|t| t.to_string())
			.unwrap_or_else(|_| name.to_string())
	}

	// An `n @ int` arm on a sum captures the unwrapped payload at offset 8.
	pub(super) fn sum_capture(&self, arm: &MatchArm, st: &Typ) -> Option<Bind> {
		let name = arm.binding.as_ref()?;
		let Typ::Sum(variants) = st else { return None };
		let [pat] = arm.patterns.as_slice() else { return None };
		let Expr::Ident(v) = &pat.0 else { return None };
		let disp = self.sum_display(v, pat.1);
		let vi = variants.iter().find(|x| x.name == disp && x.payload.len() == 1)?;
		Some((name.clone(), vi.payload[0].clone(), 8))
	}

	pub(super) fn range_pattern(
		&mut self,
		sv: Value,
		st: &Typ,
		start: Option<&Spanned<Expr>>,
		end: Option<&Spanned<Expr>>,
		span: Span,
	) -> Result<Value, Diagnostic> {
		let Typ::Int(_) = st else {
			let msg = format!("range patterns need an integer subject, got {st}");
			return Err(Diagnostic::new(msg, span.into_range()).with_label("not an integer"));
		};
		let mut cond = self.b.ins().iconst(types::I8, 1);
		for (bound, cc) in [(start, IntCC::SignedGreaterThanOrEqual), (end, IntCC::SignedLessThan)] {
			if let Some(e) = bound {
				let (bv, _) = self.check_expr(e, st)?;
				let c = self.b.ins().icmp(cc, sv, bv);
				cond = self.b.ins().band(cond, c);
			}
		}
		Ok(cond)
	}

	// Make and check enum variant.
	pub(super) fn construct_variant(
		&mut self,
		name: &str,
		variant: &str,
		args: &[Spanned<Expr>],
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
		let variants = self.enum_variants(name);
		let v = variants.iter().find(|v| v.name == variant).ok_or_else(|| {
			Diagnostic::new(format!("enum `{name}` has no variant `{variant}`"), span.into_range())
				.with_label("no such variant")
		})?;
		let (disc, payload, names) = (v.disc, v.payload.clone(), v.names.clone());
		if !names.is_empty() {
			let [(Expr::Record(entries), _)] = args else {
				let msg = format!("`{name}.{variant}` takes named fields");
				return Err(Diagnostic::new(msg, span.into_range()).with_label("use `{ field: value }`"));
			};
			let mut fields: Vec<Value> = payload.iter().map(|t| self.zero(t)).collect();
			for (k, val) in entries {
				let Expr::Ident(key) = &k.0 else {
					return Err(
						Diagnostic::new("field names must be idents", k.1.into_range()).with_label("not a field name")
					);
				};
				let idx = names.iter().position(|n| n == key).ok_or_else(|| {
					Diagnostic::new(format!("`{name}.{variant}` has no field `{key}`"), k.1.into_range())
						.with_label("no such field")
				})?;
				fields[idx] = self.check_typed(val, &payload[idx], "type mismatch")?;
			}
			let val = self.make_enum(&variants, disc, &fields);
			return Ok((val, Typ::Enum(name.to_string())));
		}
		if args.len() != payload.len() {
			let msg = format!(
				"`{name}.{variant}` takes {} field(s), got {}",
				payload.len(),
				args.len()
			);
			return Err(Diagnostic::new(msg, span.into_range()).with_label("wrong number of fields"));
		}
		let mut fields = Vec::with_capacity(args.len());
		for (arg, ft) in args.iter().zip(&payload) {
			fields.push(self.check_typed(arg, ft, "type mismatch")?);
		}
		let val = self.make_enum(&variants, disc, &fields);
		Ok((val, Typ::Enum(name.to_string())))
	}

	// Tuple struct constructor.
	pub(super) fn construct_tuple_struct(
		&mut self,
		name: &str,
		args: &[Spanned<Expr>],
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
		let typ = self.types().resolve(&TypeExpr::Name(name.to_string()), span)?;
		let Typ::TupleStruct(_, fields) = typ.clone() else {
			unreachable!("caller checked the alias");
		};
		let mut slots: Vec<Option<&Spanned<Expr>>> = vec![None; fields.len()];
		if let [(Expr::Record(entries), _)] = args
			&& fields.iter().any(|(n, _)| n.is_some())
		{
			for (k, val) in entries {
				let Expr::Ident(key) = &k.0 else {
					return Err(
						Diagnostic::new("field names must be idents", k.1.into_range()).with_label("not a field name")
					);
				};
				let idx = fields.iter().position(|(n, _)| n.as_deref() == Some(key)).ok_or_else(|| {
					Diagnostic::new(format!("`{name}` has no field `{key}`"), k.1.into_range())
						.with_label("no such field")
				})?;
				slots[idx] = Some(val);
			}
		} else if args.len() == fields.len() {
			slots = args.iter().map(Some).collect();
		} else {
			let msg = format!("`{name}` takes {} field(s), got {}", fields.len(), args.len());
			return Err(Diagnostic::new(msg, span.into_range()).with_label("wrong number of fields"));
		}
		let ptr = self.call_alloc(fields.len());
		for (i, ((_, ft), slot)) in fields.iter().zip(&slots).enumerate() {
			let v = match *slot {
				Some(arg) => self.check_typed(arg, ft, "type mismatch")?,
				None => self.zero(ft),
			};
			self.b.ins().store(MemFlags::new(), v, ptr, (i * 8) as i32);
		}
		Ok((ptr, typ))
	}

	// An expr checked against an expected type.
	pub(super) fn check_typed(&mut self, arg: &Spanned<Expr>, ft: &Typ, label: &str) -> Result<Value, Diagnostic> {
		let (fv, at) = self.check_expr(arg, ft)?;
		if at != *ft {
			let msg = format!("expected {ft}, got {at}");
			return Err(Diagnostic::new(msg, arg.1.into_range()).with_label(label));
		}
		Ok(fv)
	}

	// Evaluate `value` against an expected type.
	// Coerces variant shorthands, atoms, and `none`.
	pub(super) fn check_expr(&mut self, value: &Spanned<Expr>, target: &Typ) -> Result<TypedVal, Diagnostic> {
		if let Typ::Trait(tn) = target {
			let (val, vt) = self.expr(value)?;
			return self.make_trait_object(val, &vt, tn, value.1);
		}
		if matches!(value.0, Expr::EnumShorthand { .. } | Expr::Atom(_) | Expr::None)
			&& let Some(v) = self.coerce_lit(value, target)?
		{
			return Ok((v, target.clone()));
		}
		match &value.0 {
			Expr::Array(elems) if matches!(target, Typ::Array(_)) => {
				self.array_lit(elems, Some(&array_elem(target).clone()), value.1)
			}
			Expr::If { cond, then, els } => {
				match self.conditional(cond, then, els.as_deref(), Some(target), value.1)? {
					Some(vt) => Ok(vt),
					None => Err(
						Diagnostic::new("this `if` never produces a value", value.1.into_range())
							.with_label("every branch returns, but a value is needed here"),
					),
				}
			}
			Expr::Match {
				subject,
				arms,
				else_body,
			} => match self.match_expr(subject, arms, else_body.as_deref(), Some(target), value.1)? {
				Some(vt) => Ok(vt),
				None => Err(
					Diagnostic::new("this `match` never produces a value", value.1.into_range())
						.with_label("every arm returns, but a value is needed here"),
				),
			},
			Expr::StructLit { name, fields }
				if name == "Map" && fields.is_empty() && matches!(target, Typ::Map(..)) =>
			{
				Ok((self.call_map_new(), target.clone()))
			}
			Expr::StructLit { name, fields } => self.struct_lit(name, fields, value.1, Some(target)),
			Expr::Record(entries) => match target {
				Typ::Map(..) => self.record_lit(entries, value.1, Some(target)),
				Typ::Struct(name, _) => {
					let fields = entries
						.iter()
						.map(|(k, v)| match &k.0 {
							Expr::Ident(n) => Ok((Some(n.clone()), v.clone())),
							_ => Err(
								Diagnostic::new(format!("`{name}` fields are named by idents"), k.1.into_range())
									.with_label("not a field name"),
							),
						})
						.collect::<Result<Vec<_>, _>>()?;
					self.struct_lit(name, &fields, value.1, None)
				}
				_ => self.expr(value),
			},
			_ => {
				let (val, vt) = self.expr(value)?;
				// same member sets with different order. remap the tag into a fresh box
				if let (Typ::Sum(src), Typ::Sum(dst)) = (&vt, target)
					&& src != dst && let Some(map) = sum_remap(src, dst)
				{
					let old = self.enum_tag(src, val);
					let mut tag = self.b.ins().iconst(self.int, map[0].1);
					for &(s, d) in &map[1..] {
						let hit = self.b.ins().icmp_imm(IntCC::Equal, old, s);
						let dv = self.b.ins().iconst(self.int, d);
						tag = self.b.ins().select(hit, dv, tag);
					}
					if !enum_boxed(dst) {
						return Ok((tag, target.clone()));
					}
					let slots = enum_slots(dst);
					let ptr = self.call_alloc(slots);
					self.b.ins().store(MemFlags::new(), tag, ptr, 0);
					for i in 1..slots {
						let w = self.b.ins().load(self.int, MemFlags::new(), val, (i * 8) as i32);
						self.b.ins().store(MemFlags::new(), w, ptr, (i * 8) as i32);
					}
					return Ok((ptr, target.clone()));
				}
				if let Typ::Sum(variants) = target
					&& let Some(v) = variants.iter().find(|v| v.payload == [vt.clone()])
				{
					return Ok((self.make_enum(variants, v.disc, &[val]), target.clone()));
				}
				Ok((val, vt))
			}
		}
	}

	// Box a struct behind its vtable.
	fn make_trait_object(&mut self, val: Value, vt: &Typ, tn: &str, span: Span) -> Result<TypedVal, Diagnostic> {
		// already the right trait object, pass it through
		if matches!(vt, Typ::Trait(n) if n == tn) {
			return Ok((val, vt.clone()));
		}
		let Typ::Struct(name, _) = vt else {
			return Err(
				Diagnostic::new("only structs can be trait objects yet", span.into_range())
					.with_label(format!("`{vt}` is not a struct")),
			);
		};
		if !self.trait_impls.contains(&(name.clone(), tn.to_string())) {
			return Err(
				Diagnostic::new(format!("`{name}` doesn't implement `{tn}`"), span.into_range())
					.with_label("no matching impl"),
			);
		}
		let sym = oi_symbol(&format!("vtable_{name}_{tn}"));
		let vtable = self.data_addr(&sym);
		let boxp = self.call_alloc(2);
		self.b.ins().store(MemFlags::new(), vtable, boxp, 0);
		self.b.ins().store(MemFlags::new(), val, boxp, 8);
		Ok((boxp, Typ::Trait(tn.to_string())))
	}

	pub(super) fn float_lit(&mut self, x: f64, w: u16, span: Span) -> Result<Value, Diagnostic> {
		match w {
			32 => Ok(self.b.ins().f32const(x as f32)),
			64 => Ok(self.b.ins().f64const(x)),
			_ => Err(Diagnostic::new(
				format!("f{w} literals aren't supported by the JIT backend yet"),
				span.into_range(),
			)
			.with_label("not yet implemented")),
		}
	}

	// Record literal, lowered to a map.
	pub(super) fn record_lit(
		&mut self,
		entries: &[(Spanned<Expr>, Spanned<Expr>)],
		span: Span,
		target: Option<&Typ>,
	) -> Result<TypedVal, Diagnostic> {
		let (key_typ, mut val_typ) = match (target, entries.first()) {
			(Some(Typ::Map(k, v)), _) => ((**k).clone(), Some((**v).clone())),
			(_, Some((key, _))) => {
				let kt = match &key.0 {
					Expr::Ident(_) | Expr::String(_) => Typ::Str,
					Expr::Int(n) if i32::try_from(*n).is_ok() => Typ::Int(32),
					Expr::Int(_) => Typ::Int(64),
					Expr::Atom(_) => Typ::Atom,
					_ => unreachable!(),
				};
				(kt, None)
			}
			_ => {
				return Err(Diagnostic::new("cannot infer the type of `{}` here", span.into_range())
					.with_label("no map type is expected in this position"));
			}
		};
		let map = self.call_map_new();
		for (key, value) in entries {
			let (tag, key_bits) = match &key.0 {
				Expr::Ident(n) => self.map_key(&(Expr::String(n.clone()), key.1), &key_typ)?,
				_ => self.map_key(key, &key_typ)?,
			};
			let (val, vt) = match &val_typ {
				Some(t) => (self.check_typed(value, t, "type mismatch")?, t.clone()),
				None => self.expr(value)?,
			};
			val_typ.get_or_insert(vt);
			let bits = self.map_bits(val);
			self.call_map_set(map, tag, key_bits, bits);
		}
		let val_typ = val_typ.expect("target or first entry set it");
		Ok((map, Typ::Map(Box::new(key_typ), Box::new(val_typ))))
	}

	// Struct literal.
	// `Name {}`
	pub(super) fn struct_lit(
		&mut self,
		name: &str,
		fields: &[(Option<String>, Spanned<Expr>)],
		span: Span,
		target: Option<&Typ>,
	) -> Result<TypedVal, Diagnostic> {
		// `Self {}` inside a method resolves to the impl's type
		let name = match name {
			"Self" => self.self_type.clone().ok_or_else(|| {
				Diagnostic::new("`Self` is only valid in an impl block", span.into_range())
					.with_label("no enclosing impl")
			})?,
			_ => name.to_string(),
		};
		if self.enums.contains_key(name.as_str()) {
			if !fields.is_empty() {
				return Err(Diagnostic::new(
					format!("enum `{name}` only supports `{name}{{}}` with no fields"),
					span.into_range(),
				)
				.with_label("not a struct"));
			}
			let typ = Typ::Enum(name.clone());
			return Ok((self.zero(&typ), typ));
		}
		let struct_fields = match self.structs.get(name.as_str()) {
			Some(fields) => fields.clone(),
			None => match self.generics.structs.get(name.as_str()).cloned() {
				Some(def) => return self.generic_struct_lit(&name, def, fields, span, target),
				None => {
					return Err(Diagnostic::new(format!("unknown struct `{name}`"), span.into_range())
						.with_label("not defined"));
				}
			},
		};
		let ptr = self.struct_slot(&struct_fields)?;

		if !fields.is_empty() {
			let positional = fields[0].0.is_none();
			if positional {
				if fields.len() != struct_fields.len() {
					return Err(Diagnostic::new(
						format!(
							"`{name}` has {} fields but {} values were provided",
							struct_fields.len(),
							fields.len()
						),
						span.into_range(),
					)
					.with_label("wrong number of fields"));
				}
				for (i, (_, value)) in fields.iter().enumerate() {
					let val = self.check_typed(value, &struct_fields[i].typ, "type mismatch")?;
					let val = self.copy_in(val, &struct_fields[i].typ);
					self.b.ins().store(MemFlags::new(), val, ptr, (i * 8) as i32);
				}
			} else {
				for (field_name, value) in fields {
					let fname = field_name.as_deref().ok_or_else(|| {
						Diagnostic::new("cannot mix named and positional fields", value.1.into_range())
							.with_label("missing field name")
					})?;
					let idx = struct_fields.iter().position(|f| f.name == fname).ok_or_else(|| {
						Diagnostic::new(format!("`{name}` has no field `{fname}`"), value.1.into_range())
							.with_label("no such field")
					})?;
					let val = self.check_typed(value, &struct_fields[idx].typ, "type mismatch")?;
					let val = self.copy_in(val, &struct_fields[idx].typ);
					self.b.ins().store(MemFlags::new(), val, ptr, (idx * 8) as i32);
				}
			}
		}
		Ok((ptr, Typ::Struct(name.clone(), struct_fields)))
	}

	// Allocate a struct on the stack, initializing each field to its default.
	fn struct_slot(&mut self, struct_fields: &[FieldDef]) -> Result<Value, Diagnostic> {
		let ptr = self.stack_slot((struct_fields.len() * 8) as u32);
		for (i, f) in struct_fields.iter().enumerate() {
			let init = if let Some(default_expr) = &f.default {
				let (val, vtyp) = self.expr(default_expr)?;
				if vtyp != f.typ {
					return Err(Diagnostic::new(
						format!("default value type mismatch: expected {}, got {vtyp}", f.typ),
						default_expr.1.into_range(),
					)
					.with_label("type mismatch"));
				}
				self.copy_in(val, &f.typ)
			} else {
				self.zero(&f.typ)
			};
			self.b.ins().store(MemFlags::new(), init, ptr, (i * 8) as i32);
		}
		Ok(ptr)
	}

	// Struct literal for a generic struct.
	fn generic_struct_lit(
		&mut self,
		name: &str,
		def: GenericStructDef,
		fields: &[(Option<String>, Spanned<Expr>)],
		span: Span,
		target: Option<&Typ>,
	) -> Result<TypedVal, Diagnostic> {
		let positional = fields.first().is_some_and(|(n, _)| n.is_none());
		if positional && fields.len() != def.fields.len() {
			return Err(Diagnostic::new(
				format!(
					"`{name}` has {} fields but {} values were provided",
					def.fields.len(),
					fields.len()
				),
				span.into_range(),
			)
			.with_label("wrong number of fields"));
		}
		let mut subst = HashMap::new();
		let mut provided = Vec::with_capacity(fields.len());
		for (i, (field_name, value)) in fields.iter().enumerate() {
			let idx = match field_name {
				None if positional => i,
				None => {
					return Err(
						Diagnostic::new("cannot mix named and positional fields", value.1.into_range())
							.with_label("missing field name"),
					);
				}
				Some(fname) => def.fields.iter().position(|f| &f.name == fname).ok_or_else(|| {
					Diagnostic::new(format!("`{name}` has no field `{fname}`"), value.1.into_range())
						.with_label("no such field")
				})?,
			};
			let (val, vtyp) = self.expr(value)?;
			unify(&def.fields[idx].typ, &vtyp, &def.type_params, &mut subst, self.generics)
				.map_err(|msg| Diagnostic::new(msg, value.1.into_range()).with_label("type mismatch"))?;
			let val = self.copy_in(val, &vtyp);
			provided.push((idx, val, vtyp, value.1));
		}
		// params the field values didn't pin can come from the expected type
		if let Some(Typ::Struct(_, tfields)) = target {
			for (df, tf) in def.fields.iter().zip(tfields) {
				unify(&df.typ, &tf.typ, &def.type_params, &mut subst, self.generics).ok();
			}
		}
		if let Some(missing) = def.type_params.iter().find(|p| !subst.contains_key(&p.name)) {
			return Err(Diagnostic::new(
				format!("cannot infer type parameter `{}`", missing.name),
				span.into_range(),
			)
			.with_label("not determined by any field"));
		}
		let typ = self.types().instantiate(name, &def, &subst, span)?;
		let Typ::Struct(_, struct_fields) = &typ else {
			unreachable!()
		};
		let ptr = self.struct_slot(struct_fields)?;
		for (idx, val, vtyp, vspan) in provided {
			let expected = &struct_fields[idx].typ;
			if &vtyp != expected {
				return Err(
					Diagnostic::new(format!("expected {expected}, got {vtyp}"), vspan.into_range())
						.with_label("type mismatch"),
				);
			}
			self.b.ins().store(MemFlags::new(), val, ptr, (idx * 8) as i32);
		}
		Ok((ptr, typ))
	}

	// A stack slot's base address.
	fn stack_slot(&mut self, size: u32) -> Value {
		let slot = self
			.b
			.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, size, 0));
		self.b.ins().stack_addr(self.int, slot, 0)
	}

	pub(super) fn struct_copy(&mut self, src: Value, fields: &[FieldDef]) -> Value {
		let dst = self.stack_slot((fields.len() * 8) as u32);
		self.copy_fields(src, dst, fields);
		dst
	}

	// Copy field slots between structs.
	pub(super) fn copy_fields(&mut self, src: Value, dst: Value, fields: &[FieldDef]) {
		for (i, f) in fields.iter().enumerate() {
			let cl = cl_type(&f.typ, self.int);
			let fv = self.b.ins().load(cl, MemFlags::new(), src, (i * 8) as i32);
			let fv = self.copy_in(fv, &f.typ);
			self.b.ins().store(MemFlags::new(), fv, dst, (i * 8) as i32);
		}
	}

	pub(super) fn fixed_copy(&mut self, src: Value, elem: &Typ, n: usize) -> Value {
		let stride = self.elem_stride(elem);
		let dst = self.stack_slot((n as i64 * stride) as u32);
		for i in 0..n {
			let off = (i as i64 * stride) as i32;
			let v = self.load_elem(src, off, elem);
			self.store_elem(dst, off, elem, v);
		}
		dst
	}
}
