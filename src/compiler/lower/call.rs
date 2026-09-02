use super::*;

// Unwrap a mutable call arg.
pub(super) fn mut_inner(arg: &Spanned<Expr>) -> &Spanned<Expr> {
	match &arg.0 {
		Expr::MutArg(inner) => inner,
		_ => arg,
	}
}

// Named arguments.
pub(super) fn arg_slots<'e>(
	name: &str,
	names: &[&str],
	args: &'e [Spanned<Expr>],
	coerces: bool,
) -> Result<Option<Vec<Option<&'e Spanned<Expr>>>>, Diagnostic> {
	let Some(((Expr::Record(entries), _), pos)) = args.split_last() else {
		return Ok(None);
	};
	let named = |k: &Spanned<Expr>| matches!(&k.0, Expr::Ident(n) if !coerces || names.contains(&n.as_str()));
	if entries.is_empty() || pos.len() >= names.len() || !entries.iter().all(|(k, _)| named(k)) {
		return Ok(None);
	}
	let mut slots = vec![None; names.len()];
	for (slot, arg) in slots.iter_mut().zip(pos) {
		*slot = Some(arg);
	}
	for (key, value) in entries {
		let Expr::Ident(k) = &key.0 else { unreachable!() };
		let i = names.iter().position(|n| n == k).ok_or_else(|| {
			Diagnostic::new(format!("`{name}` has no parameter `{k}`"), key.1.into_range())
				.with_label("no such parameter")
		})?;
		if slots[i].replace(value).is_some() {
			return Err(
				Diagnostic::new(format!("`{k}` is already given"), key.1.into_range()).with_label("duplicate argument")
			);
		}
	}
	Ok(Some(slots))
}

// How a value call reaches its code.
pub(super) enum Callee {
	// a fn value
	Object(Value),
	// a vtable slot
	Addr(Value),
}

// What a mut arg lends to a callee.
pub(super) enum Lent {
	Whole(Local),
	Slice { parent: Local, lo: Value, len: Value },
}

impl<'a, M: Module> Translator<'a, M> {
	pub(super) fn import_fn(
		&mut self,
		name: &str,
		params: &[types::Type],
		ret: Option<types::Type>,
	) -> codegen::ir::FuncRef {
		let mut sig = self.module.make_signature();
		for &p in params {
			sig.params.push(AbiParam::new(p));
		}
		if let Some(r) = ret {
			sig.returns.push(AbiParam::new(r));
		}
		let id = self.module.declare_function(name, Linkage::Import, &sig).unwrap();
		self.module.declare_func_in_func(id, self.b.func)
	}

	// Type arguments only mean something on a generic fn.
	pub(super) fn check_type_args(
		&self,
		name: &str,
		key: &str,
		type_args: &[Spanned<TypeExpr>],
		span: Span,
	) -> Result<(), Diagnostic> {
		match type_args.is_empty() || self.generic_fns.contains_key(key) {
			true => Ok(()),
			false => Err(Diagnostic::new(format!("`{name}` is not generic"), span.into_range())
				.with_label("unexpected type arguments")),
		}
	}

	// Call a `pub` function of an imported module.
	pub(super) fn module_call(
		&mut self,
		module: &str,
		method: &str,
		type_args: &[Spanned<TypeExpr>],
		args: &[Spanned<Expr>],
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
		let key = format!("{module}::{method}");
		let key = self.reexports.get(&key).cloned().unwrap_or(key);
		let known = self.funcs.contains_key(&key) || self.generic_fns.contains_key(&key);
		if !self.publics.contains(&key) {
			let (msg, label) = if known {
				(format!("`{method}` is private to module `{module}`"), "not public")
			} else {
				(
					format!("module `{module}` has no function `{method}`"),
					"no such function",
				)
			};
			return Err(Diagnostic::new(msg, span.into_range()).with_label(label));
		}
		self.check_type_args(method, &key, type_args, span)?;
		if let Some(sig) = self.funcs.get(&key).cloned() {
			return self.call_sig(method, sig, None, None, args, span);
		}
		if let Some(def) = self.generic_fns.get(&key).cloned() {
			return self.call_generic(&key, &def, type_args, args, None, span);
		}
		Err(Diagnostic::new(
			format!("module `{module}` has no function `{method}`"),
			span.into_range(),
		)
		.with_label("no such function"))
	}

	// Emit a call to a resolved fn.
	pub(super) fn call_sig(
		&mut self,
		name: &str,
		sig: FnSig,
		recv: Option<Value>,
		recv_expr: Option<&Spanned<Expr>>,
		args: &[Spanned<Expr>],
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
		let self_n = recv.is_some() as usize;
		// `@params`
		let synth;
		let args = if args.len() + self_n + 1 == sig.params.len()
			&& let Some(Typ::Struct(n, _)) = sig.params.last()
			&& self
				.annotations
				.get(n)
				.is_some_and(|anns| anns.iter().any(|(e, _)| matches!(e, Expr::Ident(q) if q == "core::params")))
		{
			synth = [args, &[(Expr::Record(vec![]), span)]].concat();
			&synth[..]
		} else {
			args
		};
		let mut names: Vec<&str> = sig.args.iter().skip(self_n).map(|(n, _)| n.as_str()).collect();
		names.resize(sig.params.len() - self_n, "");
		let named = arg_slots(name, &names, args, matches!(sig.params.last(), Some(Typ::Struct(..))))?;
		if named.is_none() {
			let n_defaults = sig.args.iter().rev().take_while(|(_, d)| d.is_some()).count();
			let total = args.len() + self_n;
			if total + n_defaults < sig.params.len() || total > sig.params.len() {
				let mut want = (sig.params.len() - n_defaults - self_n).to_string();
				if n_defaults > 0 {
					want = format!("{want}..{}", sig.params.len() - self_n);
				}
				let msg = format!("`{name}` expects {want} argument(s), got {}", args.len());
				return Err(Diagnostic::new(msg, span.into_range()).with_label("wrong number of arguments"));
			}
		}
		let slots: Vec<_> = named.unwrap_or_else(|| (0..names.len()).map(|i| args.get(i)).collect());
		self.check_muts(&sig.muts, recv_expr, &slots)?;
		let fills = slots.iter().any(Option::is_none);
		let saved: Vec<_> = match fills {
			true => sig.args.iter().map(|(n, _)| (n.clone(), self.vars.remove(n))).collect(),
			false => Vec::new(),
		};
		let mut vals = Vec::with_capacity(sig.params.len());
		vals.extend(recv);
		let mut lent = Vec::new();
		for (i, want) in sig.params.iter().enumerate() {
			if i >= self_n {
				let (val, typ) = match (slots[i - self_n], sig.muts[i]) {
					(Some(arg), true) => {
						let (cell, typ, entry) = self.lend_mut(mut_inner(arg))?;
						lent.push((cell, entry));
						(cell, typ)
					}
					(Some(arg), false) => self.check_expr(mut_inner(arg), want)?,
					(None, _) => {
						let Some(default) = &sig.args[i].1 else {
							let msg = format!("`{name}` is missing argument `{}`", sig.args[i].0);
							return Err(
								Diagnostic::new(msg, span.into_range()).with_label("no value for this parameter")
							);
						};
						(
							self.check_typed(default, want, "not a valid default for this parameter")?,
							want.clone(),
						)
					}
				};
				if &typ != want {
					let at = slots[i - self_n].expect("a default is checked as it is evaluated").1;
					return Err(
						Diagnostic::new(format!("expected {want} argument, got {typ}"), at.into_range())
							.with_label("wrong argument type"),
					);
				}
				vals.push(val);
			}
			if fills {
				let var = self.b.declare_var(cl_type(want, self.int));
				self.b.def_var(var, vals[i]);
				self.vars.insert(sig.args[i].0.clone(), Local::plain(var, want.clone(), false));
			}
		}
		for (name, old) in saved {
			match old {
				Some(local) => self.vars.insert(name, local),
				None => self.vars.remove(&name),
			};
		}
		let out = self.emit_call(&sig, &vals);
		self.reload_lent(&lent);
		Ok(out)
	}

	// Pass the address of the caller's binding.
	pub(super) fn lend_mut(&mut self, inner: &Spanned<Expr>) -> Result<(Value, Typ, Lent), Diagnostic> {
		let (cur, typ, entry) = match &inner.0 {
			Expr::Slice { collection, start, end } => {
				let Expr::Ident(name) = &collection.0 else {
					unreachable!("check_muts admits only ident-based slices")
				};
				let parent = self.local(name, collection.1.into_range())?;
				let opnd = self.expr(collection)?;
				let (copy, lo, elem) = self.slice_copy(opnd, collection.1, start, end)?;
				let len = self.array_len(copy);
				(copy, Typ::Array(Box::new(elem)), Lent::Slice { parent, lo, len })
			}
			Expr::Ident(name) => {
				let local = self.local(name, inner.1.into_range())?;
				(self.read_local(&local), local.typ.clone(), Lent::Whole(local))
			}
			_ => unreachable!("check_muts admits only idents and ident-based slices"),
		};
		let slot = self.stack_slot(8);
		self.b.ins().store(MemFlags::new(), cur, slot, 0);
		Ok((slot, typ, entry))
	}

	// After a call, reload a binding.
	pub(super) fn reload_lent(&mut self, lent: &[(Value, Lent)]) {
		for (slot, entry) in lent {
			let val = self.b.ins().load(self.int, MemFlags::new(), *slot, 0);
			match entry {
				Lent::Whole(local) => self.write_local(local, val),
				Lent::Slice { parent, lo, len } => {
					let elem = array_elem(&parent.typ).clone();
					let base = self.read_local(parent);
					self.cow_array(base, &elem);
					let stride = self.elem_stride(&elem);
					let size = self.b.ins().iconst(self.int, stride);
					let func = self.import_fn(runtime::ARRAY_WRITE_BACK, &[self.int; 5], None);
					self.b.ins().call(func, &[base, *lo, *len, val, size]);
					self.release_value(val, &parent.typ);
				}
			}
		}
	}

	// Callsite mut checks.
	pub(super) fn check_muts(
		&self,
		muts: &[bool],
		recv: Option<&Spanned<Expr>>,
		args: &[Option<&Spanned<Expr>>],
	) -> Result<(), Diagnostic> {
		if let Some(re) = recv
			&& muts[0]
		{
			self.mut_place(re, "calling a `mut self` method needs a `mut` binding")?;
		}
		for (i, (&arg, &m)) in args.iter().zip(&muts[recv.is_some() as usize..]).enumerate() {
			let Some(arg) = arg else { continue };
			let name = match (&arg.0, m) {
				(Expr::MutArg(inner), true) => match &inner.0 {
					Expr::Slice { collection, .. } => {
						self.mut_place(collection, "only a mutable binding can be lent `mut`")?
					}
					_ => self.mut_place(inner, "only a mutable binding can be lent `mut`")?,
				},
				(Expr::MutArg(_), false) => {
					return Err(Diagnostic::new("this parameter is not `mut`", arg.1.into_range())
						.with_label("remove `mut` here"));
				}
				(_, true) => {
					return Err(Diagnostic::new(
						"this parameter is `mut`, missing `mut` at the callsite",
						arg.1.into_range(),
					)
					.with_label("wrap it, e.g. `f(mut x)`"));
				}
				_ => continue,
			};
			let mut touched = HashSet::new();
			let others = args.iter().enumerate().filter(|&(j, _)| j != i).filter_map(|(_, a)| *a);
			for e in recv.into_iter().chain(others) {
				e.0.idents(&mut touched);
			}
			if touched.contains(name) {
				let msg = format!("cannot use `{name}` while it is lent `mut`");
				return Err(Diagnostic::new(msg, arg.1.into_range()).with_label("borrowed exclusively for this call"));
			}
		}
		Ok(())
	}

	// Require a mutable binding place.
	fn mut_place<'e>(&self, e: &'e Spanned<Expr>, msg: &str) -> Result<&'e String, Diagnostic> {
		let Expr::Ident(name) = &e.0 else {
			return Err(Diagnostic::new(msg, e.1.into_range()).with_label("not a binding"));
		};
		if !self.local(name, e.1.into_range())?.mutable {
			return Err(Diagnostic::new(msg, e.1.into_range()).with_label("immutably bound"));
		}
		Ok(name)
	}

	// Emit the actual call instruction for a resolved fn signature.
	pub(super) fn emit_call(&mut self, sig: &FnSig, vals: &[Value]) -> TypedVal {
		let func = self.module.declare_func_in_func(sig.id, self.b.func);
		let call = self.b.ins().call(func, vals);
		let ret_val = if sig.ret.is_unit() {
			self.b.ins().iconst(self.int, 0)
		} else {
			self.b.inst_results(call)[0]
		};
		self.temp(ret_val, &sig.ret);
		(ret_val, sig.ret.clone())
	}

	// Call through a value as a function.
	pub(super) fn call_value(
		&mut self,
		name: &str,
		callee: Callee,
		typ: &Typ,
		args: &[Spanned<Expr>],
		recv: Option<Value>,
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
		let (params, ret) = match typ {
			Typ::Fn(params, ret) | Typ::Closure(params, ret, _) => (params, &**ret),
			typ => {
				return Err(Diagnostic::new(format!("`{name}` is not callable"), span.into_range())
					.with_label(format!("this is {typ}, not a function")));
			}
		};
		let self_n = recv.is_some() as usize;
		if args.len() + self_n != params.len() {
			return Err(Diagnostic::new(
				format!(
					"`{name}` expects {} argument(s), got {}",
					params.len() - self_n,
					args.len()
				),
				span.into_range(),
			)
			.with_label("wrong number of arguments"));
		}
		let muts: Vec<bool> = params.iter().map(|p| matches!(p, Typ::Mut(_))).collect();
		let slots: Vec<_> = args.iter().map(Some).collect();
		self.check_muts(&muts[self_n..], None, &slots)?;
		let mut vals = Vec::with_capacity(args.len() + self_n + 1);
		vals.extend(recv);
		let mut lent = Vec::new();
		for (arg, want) in args.iter().zip(&params[self_n..]) {
			let (val, typ) = match want {
				Typ::Mut(_) => {
					let (slot, typ, entry) = self.lend_mut(mut_inner(arg))?;
					lent.push((slot, entry));
					(slot, typ)
				}
				_ => self.check_expr(mut_inner(arg), want)?,
			};
			let want = if let Typ::Mut(inner) = want {
				inner.as_ref()
			} else {
				want
			};
			if &typ != want {
				return Err(
					Diagnostic::new(format!("expected {want} argument, got {typ}"), arg.1.into_range())
						.with_label("wrong argument type"),
				);
			}
			vals.push(val);
		}
		let mut sig = self.module.make_signature();
		sig.params.extend(params.iter().map(|t| AbiParam::new(cl_type(t, self.int))));
		let addr = match callee {
			Callee::Addr(addr) => addr,
			Callee::Object(obj) => {
				sig.params.push(AbiParam::new(self.int));
				vals.push(obj);
				self.b.ins().load(self.int, MemFlags::new(), obj, 0)
			}
		};
		let is_unit = ret.is_unit();
		if !is_unit {
			sig.returns.push(AbiParam::new(cl_type(ret, self.int)));
		}
		let sig_ref = self.b.import_signature(sig);
		let call = self.b.ins().call_indirect(sig_ref, addr, &vals);
		let ret_val = if is_unit {
			self.b.ins().iconst(self.int, 0)
		} else {
			self.b.inst_results(call)[0]
		};
		self.reload_lent(&lent);
		self.temp(ret_val, ret);
		Ok((ret_val, ret.clone()))
	}

	pub(super) fn call_concat(&mut self, a: Value, b: Value) -> Value {
		let func = self.import_fn(runtime::STR_CONCAT, &[self.int, self.int], Some(self.int));
		let call = self.b.ins().call(func, &[a, b]);
		self.b.inst_results(call)[0]
	}

	pub(super) fn call_alloc(&mut self, n: usize) -> Value {
		self.call_alloc_bytes((n * 8) as i64)
	}

	pub(super) fn call_alloc_bytes(&mut self, bytes: i64) -> Value {
		let func = self.import_fn(runtime::ALLOC, &[self.int], Some(self.int));
		let size = self.b.ins().iconst(self.int, bytes);
		let call = self.b.ins().call(func, &[size]);
		self.b.inst_results(call)[0]
	}

	// Pack a value into an i64 slot for the map's fixed width.
	pub(super) fn map_bits(&mut self, val: Value) -> Value {
		let cl = self.b.func.dfg.value_type(val);
		let iv = if cl.is_float() {
			self.b.ins().bitcast(cl_int_for_width(cl.bits() as u16), MemFlags::new(), val)
		} else {
			val
		};
		if cl.bits() < 64 {
			self.b.ins().uextend(self.int, iv)
		} else {
			iv
		}
	}

	// Recover a value's native width/kind.
	pub(super) fn unmap_bits(&mut self, val: Value, typ: &Typ) -> Value {
		let cl = cl_type(typ, self.int);
		let iv = if cl.bits() < 64 {
			self.b.ins().ireduce(cl_int_for_width(cl.bits() as u16), val)
		} else {
			val
		};
		if cl.is_float() {
			self.b.ins().bitcast(cl, MemFlags::new(), iv)
		} else {
			iv
		}
	}

	// Type-check a map index against key type `key_typ`.
	pub(super) fn map_key(
		&mut self,
		index: &Spanned<Expr>,
		key_typ: &Typ,
	) -> Result<(runtime::Tag, Value), Diagnostic> {
		let tag = map_key_tag(key_typ).ok_or_else(|| {
			Diagnostic::new(format!("{key_typ} cannot be used as a map key"), index.1.into_range())
				.with_label("unsupported key type")
		})?;
		let (val, typ) = self.check_expr(index, key_typ)?;
		if &typ != key_typ {
			return Err(
				Diagnostic::new(format!("expected {key_typ} key, got {typ}"), index.1.into_range())
					.with_label("wrong key type"),
			);
		}
		Ok((tag, self.map_bits(val)))
	}

	pub(super) fn call_map_new(&mut self) -> Value {
		let func = self.import_fn(runtime::MAP_NEW, &[], Some(self.int));
		let call = self.b.ins().call(func, &[]);
		self.b.inst_results(call)[0]
	}

	pub(super) fn call_map_get(&mut self, map: Value, tag: runtime::Tag, bits: Value) -> Value {
		let func = self.import_fn(runtime::MAP_GET, &[self.int, self.int, self.int], Some(self.int));
		let tag_v = self.b.ins().iconst(self.int, tag as i64);
		let call = self.b.ins().call(func, &[map, tag_v, bits]);
		self.b.inst_results(call)[0]
	}

	pub(super) fn call_map_set(&mut self, map: Value, tag: runtime::Tag, bits: Value, value: Value) -> Value {
		let func = self.import_fn(runtime::MAP_SET, &[self.int; 4], Some(self.int));
		let tag_v = self.b.ins().iconst(self.int, tag as i64);
		let call = self.b.ins().call(func, &[map, tag_v, bits, value]);
		self.b.inst_results(call)[0]
	}

	pub(super) fn call_map_delete(&mut self, map: Value, tag: runtime::Tag, bits: Value) -> Value {
		let func = self.import_fn(runtime::MAP_DELETE, &[self.int; 3], Some(self.int));
		let tag_v = self.b.ins().iconst(self.int, tag as i64);
		let call = self.b.ins().call(func, &[map, tag_v, bits]);
		self.b.inst_results(call)[0]
	}

	// Dispatch a trait-object method through its vtable.
	pub(super) fn dyn_call(
		&mut self,
		boxv: Value,
		tn: &str,
		method: &str,
		args: &[Spanned<Expr>],
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
		let (_, _, tmethods) = self.traits[tn];
		let Some((idx, (_, params, ret))) = trait_fns(tmethods).enumerate().find(|(_, (n, ..))| *n == method) else {
			let msg = format!("trait `{tn}` has no method `{method}`");
			return Err(Diagnostic::new(msg, span.into_range()).with_label("no such method"));
		};
		// the receiver slot is the erased data pointer, the rest resolve like any signature
		let mut typs = vec![Typ::Trait(tn.into())];
		for p in params.iter().skip(1) {
			typs.push(self.types().resolve(&p.typ, p.span)?);
		}
		let ret = match ret {
			Some((te, s)) => self.types().resolve(te, *s)?,
			None => Typ::unit(),
		};
		let vtable = self.b.ins().load(self.int, MemFlags::new(), boxv, 0);
		let data = self.b.ins().load(self.int, MemFlags::new(), boxv, 8);
		let fnptr = self.b.ins().load(self.int, MemFlags::new(), vtable, (idx * 8) as i32);
		let typ = Typ::Fn(typs, Box::new(ret));
		self.call_value(method, Callee::Addr(fnptr), &typ, args, Some(data), span)
	}

	// Dyn-dispatch `message()` on a boxed `Error`.
	pub(super) fn error_message(&mut self, boxv: Value) -> Value {
		let Ok((s, _)) = self.dyn_call(boxv, "core::Error", "message", &[], (0..0).into()) else {
			unreachable!("core::Error always has `message`")
		};
		s
	}

	// Read a required trait field.
	pub(super) fn trait_field(
		&mut self,
		boxv: Value,
		tn: &str,
		field: &str,
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
		let (_, tfields, tmethods) = self.traits[tn];
		let Some(idx) = tfields.iter().position(|f| f.name == field) else {
			let msg = format!("trait `{tn}` has no field `{field}`");
			return Err(Diagnostic::new(msg, span.into_range()).with_label("no such field"));
		};
		let ftyp = self.types().resolve(&tfields[idx].typ, tfields[idx].span)?;
		let vtable = self.b.ins().load(self.int, MemFlags::new(), boxv, 0);
		let data = self.b.ins().load(self.int, MemFlags::new(), boxv, 8);
		let m = trait_fns(tmethods).count();
		// slot offset lives after the method pointers in the vtable
		let off = self.b.ins().load(self.int, MemFlags::new(), vtable, ((m + idx) * 8) as i32);
		let func = self.import_fn(runtime::TRAIT_FIELD, &[self.int, self.int], Some(self.int));
		let call = self.b.ins().call(func, &[data, off]);
		let addr = self.b.inst_results(call)[0];
		let v = self.b.ins().load(cl_type(&ftyp, self.int), MemFlags::new(), addr, 0);
		Ok((v, ftyp))
	}
}
