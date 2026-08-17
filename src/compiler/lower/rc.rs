use super::*;

// Ownership bookkeeping.
// Every ref has one owner: a named binding, a container slot, or the scope that produced it.
// Owned values register in the innermost scope and release when it exits.

impl<'a> Translator<'a> {
	// Check whether a struct has Drop trait.
	pub(super) fn is_resource(&self, typ: &Typ) -> bool {
		matches!(typ, Typ::Struct(name, _) if self.trait_impls.contains(&(name.clone(), "Drop".into())))
	}

	// Transfer ownership of a resource.
	pub fn move_resource(&mut self, e: &Spanned<Expr>, typ: &Typ) -> Result<(), Diagnostic> {
		if self.is_resource(typ)
			&& let Expr::Ident(n) = &e.0
		{
			let local = self.local(n, e.1.into_range())?;
			self.move_local(n, &local, e.1.into_range())?;
		}
		Ok(())
	}

	// Emit one release for an owned value.
	pub(super) fn release_value(&mut self, val: Value, typ: &Typ) {
		if let Some((_, release)) = handle_fns(typ) {
			let func = self.import_fn(release, &[self.int], None);
			self.b.ins().call(func, &[val]);
		} else if let Typ::Struct(name, fields) = typ {
			if self.is_resource(typ)
				&& let Some(sig) = self.funcs.get(&format!("{name}.drop")).cloned()
			{
				self.emit_call(&sig, &[val]);
			}
			for (i, f) in fields.clone().iter().enumerate() {
				if releasable(&f.typ) {
					let cl = cl_type(&f.typ, self.int);
					let fv = self.b.ins().load(cl, MemFlags::new(), val, (i * 8) as i32);
					self.release_value(fv, &f.typ);
				}
			}
		}
	}

	// The address of a struct's trace descriptor symbol.
	pub(super) fn trace_desc(&mut self, name: &str, fields: &[FieldDef]) -> Value {
		if self.desc_data(name, fields).is_none() {
			return self.b.ins().iconst(self.int, 0);
		}
		self.data_addr(&oi_symbol(&format!("{name}#trace")))
	}

	// Define trace descriptor on first use.
	fn desc_data(&mut self, name: &str, fields: &[FieldDef]) -> Option<DataId> {
		if let Some(&id) = self.descs.get(name) {
			return Some(id);
		}
		let mut words = vec![0i64];
		let mut relocs = Vec::new();
		for (i, f) in fields.iter().enumerate() {
			let off = ((i * 8) as i64) << 1;
			if ref_like(&f.typ) {
				words.push(off);
			} else if let Typ::Struct(n, sub) = &f.typ
				&& let Some(child) = self.desc_data(n, sub)
			{
				words.push(off | 1);
				relocs.push((words.len() * 8, child));
				words.push(0);
			}
		}
		words[0] = (words.len() - 1 - relocs.len()) as i64;
		if words[0] == 0 {
			return None;
		}
		let mut desc = DataDescription::new();
		desc.define(words.iter().flat_map(|w| w.to_le_bytes()).collect());
		for (off, child) in relocs {
			let gv = self.module.declare_data_in_data(child, &mut desc);
			desc.write_data_addr(off as u32, gv, 0);
		}
		let sym = oi_symbol(&format!("{name}#trace"));
		let id = self
			.module
			.declare_data(&sym, Linkage::Local, false, false)
			.expect("declare trace");
		self.module.define_data(id, &desc).expect("define trace");
		self.descs.insert(name.to_string(), id);
		Some(id)
	}

	// Register a producer's fresh handle with the innermost scope.
	pub(super) fn temp(&mut self, val: Value, typ: &Typ) {
		if handle_fns(typ).is_some() {
			let var = self.b.declare_var(self.int);
			self.b.def_var(var, val);
			self.scopes.last_mut().expect("scope").push((var, typ.clone()));
		}
	}

	// Declare a named binding that owns its value.
	pub fn bind_local(&mut self, name: &str, val: Value, typ: Typ, mutable: bool) {
		let var = self.b.declare_var(self.b.func.dfg.value_type(val));
		self.b.def_var(var, val);
		if releasable(&typ) || self.is_resource(&typ) {
			self.scopes.last_mut().expect("scope").push((var, typ.clone()));
		}
		self.vars.insert(name.to_string(), Local::plain(var, typ, mutable));
	}

	// Emit releases for every scope deeper than `depth`.
	pub(super) fn release_scopes(&mut self, depth: usize) {
		for s in (depth..self.scopes.len()).rev() {
			for i in (0..self.scopes[s].len()).rev() {
				let (var, t) = self.scopes[s][i].clone();
				let v = self.b.use_var(var);
				self.release_value(v, &t);
			}
		}
	}

	// Transfer ownership of a local binding.
	pub fn move_local(&mut self, name: &str, local: &Local, span: Range<usize>) -> Result<Value, Diagnostic> {
		if releasable(&local.typ) || self.is_resource(&local.typ) {
			let depth = self
				.scopes
				.iter()
				.position(|s| s.iter().any(|(v, _)| *v == local.var))
				.ok_or_else(|| {
					Diagnostic::new(format!("cannot move `{name}`, it is borrowed here"), span.clone())
						.with_label("only an owned binding can be moved")
				})?;
			if let Some(frame) = self.loops.last()
				&& depth < frame.depth
			{
				return Err(
					Diagnostic::new(format!("cannot move `{name}` out of the enclosing loop"), span)
						.with_label("would be moved again on the next iteration"),
				);
			}
			self.scopes[depth].retain(|(v, _)| *v != local.var);
		}
		self.vars.remove(name);
		Ok(self.read_local(local))
	}

	// A bind takes its own copy.
	// Structs deep-copy and handles bump.
	pub(super) fn copy_bind(&mut self, val: Value, typ: &Typ) -> Value {
		match typ {
			Typ::Struct(_, fields) => {
				let fields = fields.clone();
				self.struct_copy(val, &fields)
			}
			_ => self.copy_in(val, typ),
		}
	}
}

pub(super) fn releasable(typ: &Typ) -> bool {
	match typ {
		Typ::Struct(_, fields) => fields.iter().any(|f| releasable(&f.typ)),
		_ => handle_fns(typ).is_some(),
	}
}

// The runtime share/release fns for rc'd types.
pub(super) fn handle_fns(typ: &Typ) -> Option<(&'static str, &'static str)> {
	match typ {
		Typ::Array(_) => Some((runtime::ARRAY_SHARE, runtime::ARRAY_RELEASE)),
		Typ::Map(..) => Some((runtime::MAP_SHARE, runtime::MAP_RELEASE)),
		t if ref_like(t) => Some((runtime::REF_SHARE, runtime::REF_RELEASE)),
		_ => None,
	}
}

// Is `typ` a `?&T`?
pub(super) fn opt_ref(typ: &Typ) -> bool {
	matches!(typ, Typ::Option(i) if matches!(&**i, Typ::Ref(_)))
}

// Is type a ref pointer?
pub(super) fn ref_like(typ: &Typ) -> bool {
	matches!(typ, Typ::Ref(_)) || opt_ref(typ)
}
