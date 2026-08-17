use super::*;

// What `emit_eq` compares directly.
fn comparable(t: &Typ) -> bool {
	use Typ::*;
	t.is_enumish()
		|| matches!(
			t,
			Int(_) | UInt(_) | ISize | USize | Bool | Atom | Float(_) | Str | Error
		)
}

impl<'a> Translator<'a> {
	pub(super) fn emit_eq(&mut self, a: Value, b: Value, typ: &Typ) -> Value {
		match typ {
			Typ::Float(_) => self.b.ins().fcmp(FloatCC::Equal, a, b),
			Typ::Str | Typ::Error => {
				let func = self.import_fn(runtime::STR_EQ, &[self.int, self.int], Some(self.int));
				let call = self.b.ins().call(func, &[a, b]);
				self.b.inst_results(call)[0]
			}
			_ => self.b.ins().icmp(IntCC::Equal, a, b),
		}
	}

	// Compare two boxed enums.
	// Checks that tags match, and for variants that every field matches
	pub(super) fn emit_enum_eq(&mut self, a: Value, b: Value, typ: &Typ) -> Value {
		let variants = self.variants_of(typ);
		let ta = self.enum_tag(typ, a);
		let tb = self.enum_tag(typ, b);
		let tags_eq = self.b.ins().icmp(IntCC::Equal, ta, tb);
		let eq = self.b.declare_var(types::I8);
		self.b.def_var(eq, tags_eq);
		let merge = self.b.create_block();
		for v in variants.iter().filter(|v| !v.payload.is_empty()) {
			let disc = self.b.ins().iconst(self.int, v.disc);
			let same = self.b.ins().icmp(IntCC::Equal, ta, disc);
			let hit = self.b.ins().band(tags_eq, same);
			let (body, next) = (self.b.create_block(), self.b.create_block());
			self.b.ins().brif(hit, body, &[], next, &[]);
			self.b.seal_block(body);
			self.b.seal_block(next);
			self.b.switch_to_block(body);
			for (i, ft) in v.payload.iter().enumerate() {
				let fa = self
					.b
					.ins()
					.load(cl_type(ft, self.int), MemFlags::new(), a, ((i + 1) * 8) as i32);
				let fb = self
					.b
					.ins()
					.load(cl_type(ft, self.int), MemFlags::new(), b, ((i + 1) * 8) as i32);
				let fe = self.emit_eq(fa, fb, ft);
				let fe = self.b.ins().icmp_imm(IntCC::NotEqual, fe, 0);
				let prev = self.b.use_var(eq);
				let acc = self.b.ins().band(prev, fe);
				self.b.def_var(eq, acc);
			}
			self.b.ins().jump(merge, &[]);
			self.b.switch_to_block(next);
		}
		self.b.ins().jump(merge, &[]);
		self.b.switch_to_block(merge);
		self.b.seal_block(merge);
		self.b.use_var(eq)
	}

	// The `Eq` fill claimed for `name`, if any.
	fn eq_fill(&self, name: &str) -> Option<FnSig> {
		let claimed = self.trait_impls.contains(&(name.to_string(), "Eq".into()));
		self.funcs
			.get(&format!("{name}.eq"))
			.cloned()
			.filter(|s| claimed && s.params.len() == 2)
	}

	// Compare two structs field by field.
	pub(super) fn emit_struct_eq(&mut self, a: Value, b: Value, typ: &Typ, span: Span) -> Result<Value, Diagnostic> {
		let Typ::Struct(name, fields) = typ else {
			unreachable!("emit_struct_eq on {typ}")
		};
		let mut acc = self.b.ins().iconst(types::I8, 1);
		for (i, f) in fields.iter().enumerate() {
			let cl = cl_type(&f.typ, self.int);
			let fa = self.b.ins().load(cl, MemFlags::new(), a, (i * 8) as i32);
			let fb = self.b.ins().load(cl, MemFlags::new(), b, (i * 8) as i32);
			let eq = match &f.typ {
				Typ::Struct(n, _) => match self.eq_fill(n) {
					Some(sig) => self.emit_call(&sig, &[fa, fb]).0,
					None => self.emit_struct_eq(fa, fb, &f.typ, span)?,
				},
				t if t.is_enumish() && enum_boxed(&self.variants_of(t)) && !rc::opt_ref(t) => {
					self.emit_enum_eq(fa, fb, t)
				}
				t if comparable(t) => self.emit_eq(fa, fb, t),
				ft => {
					let msg = format!("cannot compare {name}: field `{}` is {ft}", f.name);
					return Err(Diagnostic::new(msg, span.into_range())
						.with_label(format!("claim `Eq` for `{name}` to define equality")));
				}
			};
			let eq = self.b.ins().icmp_imm(IntCC::NotEqual, eq, 0);
			acc = self.b.ins().band(acc, eq);
		}
		Ok(acc)
	}

	// Cast int-like to int-like.
	// ref: https://github.com/rust-lang/rustc_codegen_cranelift/blob/main/src/cast.rs
	pub(super) fn intcast(&mut self, val: Value, to: types::Type, signed: bool) -> Value {
		let from = self.b.func.dfg.value_type(val);
		if from == to {
			val
		} else if from.bits() > to.bits() {
			self.b.ins().ireduce(to, val)
		} else if signed {
			self.b.ins().sextend(to, val)
		} else {
			self.b.ins().uextend(to, val)
		}
	}

	// Sign-extend the low `w` bits of `val` within its container.
	// NOTE: noop for standard cranelift widths (8, 16, 32, 64).
	pub(super) fn reduce_int(&mut self, val: Value, w: u16) -> Value {
		let cl = cl_type(&Typ::Int(w), self.int);
		let shift = cl.bits() as i64 - w as i64;
		if shift == 0 {
			return val;
		}
		let shift_v = self.b.ins().iconst(cl, shift);
		let up = self.b.ins().ishl(val, shift_v);
		self.b.ins().sshr(up, shift_v)
	}

	// Zero-extend (mask) `val` to exactly `w` bits within its Cranelift container.
	pub(super) fn reduce_uint(&mut self, val: Value, w: u16) -> Value {
		let cl = cl_type(&Typ::UInt(w), self.int);
		if cl.bits() as u16 == w {
			return val;
		}
		let mask = ((1u64 << w) - 1) as i64;
		let mask_v = self.b.ins().iconst(cl, mask);
		self.b.ins().band(val, mask_v)
	}

	// Promote ints to larger-width ints and floats.
	fn promote(&mut self, lv: Value, lt: Typ, rv: Value, rt: Typ) -> (Value, Typ, Value, Typ) {
		let ints = |t: &Typ| matches!(t, Typ::Int(_) | Typ::ISize | Typ::UInt(_) | Typ::USize);
		let common = match (&lt, &rt) {
			(Typ::Int(a), Typ::Int(b)) => Typ::Int(*a.max(b)),
			(Typ::UInt(a), Typ::UInt(b)) => Typ::UInt(*a.max(b)),
			(Typ::Float(a), Typ::Float(b)) => Typ::Float(*a.max(b)),
			(Typ::Float(w), o) | (o, Typ::Float(w)) if ints(o) => Typ::Float(*w),
			_ => return (lv, lt, rv, rt),
		};
		let (lv, rv) = (self.numcast(lv, &lt, &common), self.numcast(rv, &rt, &common));
		(lv, common.clone(), rv, common)
	}

	// Cast a numeric value to a wider (or same) numeric type.
	fn numcast(&mut self, val: Value, from: &Typ, to: &Typ) -> Value {
		if from == to {
			return val;
		}
		let cl = cl_type(to, self.int);
		match (from, to) {
			(Typ::Float(_), _) => self.b.ins().fpromote(cl, val),
			(Typ::UInt(_) | Typ::USize, Typ::Float(_)) => self.b.ins().fcvt_from_uint(cl, val),
			(_, Typ::Float(_)) => self.b.ins().fcvt_from_sint(cl, val),
			(Typ::UInt(_), _) => self.intcast(val, cl, false),
			_ => self.intcast(val, cl, true),
		}
	}

	pub(super) fn binop(
		&mut self,
		op: BinOp,
		l: &Spanned<Expr>,
		r: &Spanned<Expr>,
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
		let (lv, lt) = self.expr(l)?;

		if let Typ::Struct(name, _) = &lt {
			// overloads
			let tn = match op {
				BinOp::Add => "Add",
				BinOp::Sub => "Sub",
				BinOp::Mul => "Mul",
				BinOp::Div => "Div",
				BinOp::Mod => "Mod",
				_ => unreachable!("non-arithmetic op in binop"),
			};
			let claimed = self.trait_impls.contains(&(name.clone(), tn.to_string()));
			let sig = self.funcs.get(&format!("{name}.{}", tn.to_ascii_lowercase())).cloned();
			let Some(sig) = sig.filter(|s| claimed && s.params.len() == 2) else {
				return Err(
					Diagnostic::new(format!("cannot apply `{op}` to {lt}"), span.into_range())
						.with_label(format!("implement `{tn}` for `{name}` to overload `{op}`")),
				);
			};
			let (rv, rt) = self.check_expr(r, &sig.params[1])?;
			if rt != sig.params[1] {
				return Err(Diagnostic::new(
					format!("expected {} argument, got {rt}", sig.params[1]),
					r.1.into_range(),
				)
				.with_label("wrong argument type"));
			}
			return Ok(self.emit_call(&sig, &[lv, rv]));
		}
		let (rv, rt) = self.expr(r)?;

		// string concatenation
		if let (BinOp::Add, Typ::Str, Typ::Str) = (op, &lt, &rt) {
			return Ok((self.call_concat(lv, rv), Typ::Str));
		}
		let (lv, lt, rv, rt) = self.promote(lv, lt, rv, rt);

		#[derive(Clone, Copy)]
		enum NumKind {
			Int,
			UInt,
			Float,
		}
		let kind = match (&lt, &rt) {
			(Typ::Int(lw), Typ::Int(rw)) if lw == rw => NumKind::Int,
			(Typ::ISize, Typ::ISize) => NumKind::Int,
			(Typ::UInt(lw), Typ::UInt(rw)) if lw == rw => NumKind::UInt,
			(Typ::USize, Typ::USize) => NumKind::UInt,
			(Typ::Float(lw), Typ::Float(rw)) if lw == rw => NumKind::Float,
			_ => {
				return Err(
					Diagnostic::new(format!("cannot apply `{op}` to {lt} and {rt}"), span.into_range())
						.with_label("operands have mismatched types"),
				);
			}
		};
		if let (BinOp::Mod, NumKind::Float) = (op, kind) {
			// TODO: cranelift has no float remainder
			return Err(
				Diagnostic::new("`%` is not yet supported on floats".to_string(), span.into_range())
					.with_label("only integer operands"),
			);
		}
		let b = self.b.ins();
		let out = match (op, kind) {
			(BinOp::Add, NumKind::Float) => b.fadd(lv, rv),
			(BinOp::Add, _) => b.iadd(lv, rv),
			(BinOp::Sub, NumKind::Float) => b.fsub(lv, rv),
			(BinOp::Sub, _) => b.isub(lv, rv),
			(BinOp::Mul, NumKind::Float) => b.fmul(lv, rv),
			(BinOp::Mul, _) => b.imul(lv, rv),
			(BinOp::Div, NumKind::Float) => b.fdiv(lv, rv),
			(BinOp::Div, NumKind::UInt) => b.udiv(lv, rv),
			(BinOp::Div, NumKind::Int) => b.sdiv(lv, rv),
			(BinOp::Mod, NumKind::Float) => unreachable!("float `%` rejected above"),
			(BinOp::Mod, NumKind::UInt) => b.urem(lv, rv),
			(BinOp::Mod, NumKind::Int) => b.srem(lv, rv),
			_ => unreachable!("non-arithmetic op in binop"),
		};
		// For non-standard widths, wrap the result back to the declared bit width.
		let out = match &lt {
			Typ::Int(w) if cl_type(&Typ::Int(*w), self.int).bits() as u16 != *w => self.reduce_int(out, *w),
			Typ::UInt(w) if cl_type(&Typ::UInt(*w), self.int).bits() as u16 != *w => self.reduce_uint(out, *w),
			_ => out,
		};
		Ok((out, lt))
	}

	pub(super) fn cmp(
		&mut self,
		icc: IntCC,
		fcc: FloatCC,
		l: &Spanned<Expr>,
		r: &Spanned<Expr>,
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
		// evaluate the typed/pinned side first so a `.variant` shorthand can borrow its enum type
		let ((lv, lt), (rv, rt)) = if let Expr::EnumShorthand { .. } = &l.0 {
			let (rv, rt) = self.expr(r)?;
			(self.check_expr(l, &rt)?, (rv, rt))
		} else {
			let (lv, lt) = self.expr(l)?;
			let rhs = self.check_expr(r, &lt)?;
			((lv, lt), rhs)
		};
		let (lv, lt, rv, rt) = self.promote(lv, lt, rv, rt);

		// () == ()
		if let (Typ::Tuple(lf), Typ::Tuple(rf)) = (&lt, &rt)
			&& lf.is_empty()
			&& rf.is_empty()
		{
			let result = match icc {
				IntCC::Equal => self.b.ins().iconst(self.int, 1),
				IntCC::NotEqual => self.b.ins().iconst(self.int, 0),
				_ => {
					return Err(
						Diagnostic::new("unit type `()` only supports `==` and `!=`", span.into_range())
							.with_label("unsupported comparison"),
					);
				}
			};
			return Ok((result, Typ::Bool));
		}

		let icc = if matches!((&lt, &rt), (Typ::UInt(_), Typ::UInt(_)) | (Typ::USize, Typ::USize)) {
			unsigned_cc(icc)
		} else {
			icc
		};
		let raw = match (&lt, &rt) {
			(Typ::Int(_), Typ::Int(_))
			| (Typ::UInt(_), Typ::UInt(_))
			| (Typ::ISize, Typ::ISize)
			| (Typ::USize, Typ::USize)
			| (Typ::Bool, Typ::Bool)
			| (Typ::Atom, Typ::Atom) => self.b.ins().icmp(icc, lv, rv),
			(l, r) if l == r && l.is_enumish() => {
				if !enum_boxed(&self.variants_of(l)) || rc::opt_ref(l) {
					self.b.ins().icmp(icc, lv, rv)
				} else if let IntCC::Equal | IntCC::NotEqual = icc {
					let eq = self.emit_enum_eq(lv, rv, l);
					if icc == IntCC::Equal {
						eq
					} else {
						self.b.ins().icmp_imm(IntCC::Equal, eq, 0)
					}
				} else {
					return Err(Diagnostic::new(
						format!("only `==`&`!=` are supported because `{l}` has payloads"),
						span.into_range(),
					)
					.with_label("ordering needs a plain value"));
				}
			}
			(Typ::Float(_), Typ::Float(_)) => self.b.ins().fcmp(fcc, lv, rv),
			(Typ::Str, Typ::Str) if icc == IntCC::Equal || icc == IntCC::NotEqual => {
				let eq = self.emit_eq(lv, rv, &Typ::Str);
				// emit_eq returns 1 for equal, invert for Ne
				// wrap in icmp so uextend below works consistently
				if icc == IntCC::NotEqual {
					self.b.ins().icmp_imm(IntCC::Equal, eq, 0)
				} else {
					self.b.ins().icmp_imm(IntCC::NotEqual, eq, 0)
				}
			}
			(Typ::Struct(name, _), _) if lt == rt && (icc == IntCC::Equal || icc == IntCC::NotEqual) => {
				let eq = match self.eq_fill(name) {
					Some(sig) => self.emit_call(&sig, &[lv, rv]).0,
					None => self.emit_struct_eq(lv, rv, &lt, span)?,
				};
				if icc == IntCC::NotEqual {
					self.b.ins().icmp_imm(IntCC::Equal, eq, 0)
				} else {
					self.b.ins().icmp_imm(IntCC::NotEqual, eq, 0)
				}
			}
			_ => {
				return Err(
					Diagnostic::new(format!("cannot compare {lt} and {rt}"), span.into_range())
						.with_label("operands have mismatched types"),
				);
			}
		};
		let out = self.b.ins().uextend(self.int, raw);
		Ok((out, Typ::Bool))
	}

	// `lhs in rhs`.
	pub(super) fn in_op(&mut self, lhs: &Spanned<Expr>, rhs: &Spanned<Expr>) -> Result<TypedVal, Diagnostic> {
		let (rhs_val, rhs_typ) = self.expr(rhs)?;

		// substring
		if rhs_typ == Typ::Str {
			let (lhs_val, lhs_typ) = self.expr(lhs)?;
			if lhs_typ != Typ::Str {
				return Err(
					Diagnostic::new(format!("cannot search {lhs_typ} in Str"), lhs.1.into_range())
						.with_label("type mismatch: value must be Str"),
				);
			}
			let func = self.import_fn(runtime::STR_CONTAINS, &[self.int, self.int], Some(self.int));
			let call = self.b.ins().call(func, &[rhs_val, lhs_val]);
			return Ok((self.b.inst_results(call)[0], Typ::Bool));
		}

		let elem = match rhs_typ {
			Typ::Array(ref e) => (**e).clone(),
			_ => {
				return Err(Diagnostic::new(
					format!("right side of `in` must be an array or Str, got {rhs_typ}"),
					rhs.1.into_range(),
				)
				.with_label("not an array or string"));
			}
		};
		let (val, val_typ) = self.expr(lhs)?;
		if val_typ != elem {
			return Err(
				Diagnostic::new(format!("cannot search {val_typ} in {elem} array"), lhs.1.into_range())
					.with_label("type mismatch"),
			);
		}

		let arr = rhs_val;
		let len = self.array_len(arr);
		let data = self.array_data(arr);

		let found = self.b.declare_var(self.int);
		let i = self.b.declare_var(self.int);
		let zero = self.b.ins().iconst(self.int, 0);
		self.b.def_var(found, zero);
		self.b.def_var(i, zero);

		let (header, body, found_block, continue_block, exit) = (
			self.b.create_block(),
			self.b.create_block(),
			self.b.create_block(),
			self.b.create_block(),
			self.b.create_block(),
		);
		self.b.ins().jump(header, &[]);

		self.b.switch_to_block(header);
		let iv = self.b.use_var(i);
		let more = self.b.ins().icmp(IntCC::SignedLessThan, iv, len);
		self.b.ins().brif(more, body, &[], exit, &[]);
		self.b.seal_block(body);

		self.b.switch_to_block(body);
		let iv = self.b.use_var(i);
		let elem_val = self.load_nth(data, iv, &elem);
		let equal = self.emit_eq(val, elem_val, &elem);
		self.b.ins().brif(equal, found_block, &[], continue_block, &[]);
		self.b.seal_block(found_block);
		self.b.seal_block(continue_block);

		self.b.switch_to_block(found_block);
		let one = self.b.ins().iconst(self.int, 1);
		self.b.def_var(found, one);
		self.b.ins().jump(exit, &[]);
		self.b.seal_block(exit);

		self.b.switch_to_block(continue_block);
		let iv = self.b.use_var(i);
		let next = self.b.ins().iadd_imm(iv, 1);
		self.b.def_var(i, next);
		self.b.ins().jump(header, &[]);
		self.b.seal_block(header);

		self.b.switch_to_block(exit);
		Ok((self.b.use_var(found), Typ::Bool))
	}

	// Short-circuits. `&&` only evaluates the right side when the left is true, and `||` does the inverse.
	pub(super) fn logical(&mut self, and: bool, l: &Spanned<Expr>, r: &Spanned<Expr>) -> Result<TypedVal, Diagnostic> {
		let (lv, lt) = self.expr(l)?;
		if lt != Typ::Bool {
			return Err(Diagnostic::new(format!("expected Bool, got {lt}"), l.1.into_range())
				.with_label("logical operators need Bool operands"));
		}

		// result defaults to the short-circuit value
		let result = self.b.declare_var(self.int);
		let short = self.b.ins().iconst(self.int, if and { 0 } else { 1 });
		self.b.def_var(result, short);

		let rhs_block = self.b.create_block();
		let merge = self.b.create_block();
		let (then, els) = if and { (rhs_block, merge) } else { (merge, rhs_block) };
		self.b.ins().brif(lv, then, &[], els, &[]);

		self.b.switch_to_block(rhs_block);
		self.b.seal_block(rhs_block);
		let (rv, rt) = self.expr(r)?;
		if rt != Typ::Bool {
			return Err(Diagnostic::new(format!("expected Bool, got {rt}"), r.1.into_range())
				.with_label("logical operators need Bool operands"));
		}
		self.b.def_var(result, rv);
		self.b.ins().jump(merge, &[]);

		self.b.switch_to_block(merge);
		self.b.seal_block(merge);
		Ok((self.b.use_var(result), Typ::Bool))
	}
}
