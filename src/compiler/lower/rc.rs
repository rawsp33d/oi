use super::*;

// Ownership bookkeeping.
// Every ref has one owner: a named binding, a container slot, or the scope that produced it.
// Owned values register in the innermost scope and release when it exits.

impl<'a> Translator<'a> {
	// Emit one release for an owned value.
	pub(super) fn release_value(&mut self, val: Value, typ: &Typ) {
		match typ {
			Typ::Array(_) => self.call_release(runtime::ARRAY_RELEASE, val),
			Typ::Map(..) => self.call_release(runtime::MAP_RELEASE, val),
			Typ::Struct(_, fields) => {
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
		if releasable(&typ) {
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

	// A bind takes its own copy.
	// Structs deep-copy, handles bump.
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
