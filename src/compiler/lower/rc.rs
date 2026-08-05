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
		match typ {
			Typ::Array(_) => self.call_release(runtime::ARRAY_RELEASE, val),
			Typ::Map(..) => self.call_release(runtime::MAP_RELEASE, val),
			Typ::Struct(name, fields) => {
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
			_ => {}
		}
	}

	fn call_release(&mut self, sym: &str, val: Value) {
		let func = self.import_fn(sym, &[self.int], None);
		self.b.ins().call(func, &[val]);
	}

	// Register a producer's fresh handle with the innermost scope.
	pub(super) fn temp(&mut self, val: Value, typ: &Typ) {
		if matches!(typ, Typ::Array(_) | Typ::Map(..)) {
			let var = self.b.declare_var(self.int);
			self.b.def_var(var, val);
			self.scopes.last_mut().expect("scope").push((var, typ.clone()));
		}
	}

	// Declare a named binding that owns its value.
	pub(super) fn bind_local(&mut self, name: &str, val: Value, typ: Typ, mutable: bool) {
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
		Typ::Array(_) | Typ::Map(..) => true,
		Typ::Struct(_, fields) => fields.iter().any(|f| releasable(&f.typ)),
		_ => false,
	}
}
