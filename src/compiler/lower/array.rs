use super::*;

impl<'a> Translator<'a> {
	// array handle: { data @ 0, len @ 8, cap @ 16 }
	pub(super) fn array_data(&mut self, header: Value) -> Value {
		self.b.ins().load(self.int, MemFlags::new(), header, 0)
	}

	pub(super) fn array_len(&mut self, header: Value) -> Value {
		self.b.ins().load(self.int, MemFlags::new(), header, 8)
	}

	pub(super) fn array_cap(&mut self, header: Value) -> Value {
		self.b.ins().load(self.int, MemFlags::new(), header, 16)
	}

	// Build a fresh array handle, owned by the enclosing scope.
	pub(super) fn make_array(&mut self, data: Value, len: Value, typ: &Typ) -> Value {
		let header = self.call_alloc(3);
		self.b.ins().store(MemFlags::new(), data, header, 0);
		self.b.ins().store(MemFlags::new(), len, header, 8);
		self.b.ins().store(MemFlags::new(), len, header, 16);
		self.temp(header, typ);
		header
	}

	// Build an array literal.
	pub(super) fn array_lit(
		&mut self,
		elems: &[Spanned<Expr>],
		want: Option<&Typ>,
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
		let mut elem = want.cloned();
		let mut vals = Vec::with_capacity(elems.len());
		for e in elems {
			let (val, typ) = match &elem {
				Some(t) => self.check_expr(e, t)?,
				None => self.expr(e)?,
			};
			closure_escape(&typ, e.1.into_range(), "stored in an array")?;
			match &elem {
				Some(t) if t != &typ => {
					let msg = format!("array elements must share a type: expected {t}, got {typ}");
					return Err(Diagnostic::new(msg, e.1.into_range()).with_label("mismatched element type"));
				}
				_ => elem = Some(typ),
			}
			vals.push(val);
		}
		let Some(elem) = elem else {
			return Err(
				Diagnostic::new("empty array literals aren't supported yet", span.into_range())
					.with_label("needs at least one element to infer its type"),
			);
		};
		let size = self.elem_stride(&elem);
		let base = self.call_alloc_bytes(vals.len() as i64 * size + 8);
		let one = self.b.ins().iconst(self.int, 1);
		self.b.ins().store(MemFlags::new(), one, base, 0);
		let data = self.b.ins().iadd_imm(base, 8);
		for (i, val) in vals.into_iter().enumerate() {
			let val = self.copy_in(val, &elem);
			self.store_elem(data, (i as i64 * size) as i32, &elem, val);
		}
		let len = self.b.ins().iconst(self.int, elems.len() as i64);
		let typ = Typ::Array(Box::new(elem));
		Ok((self.make_array(data, len, &typ), typ))
	}

	// Copy-in point for rc'd handles.
	// RC bump.
	// The underlying buffer clone waits for a write.
	pub(super) fn copy_in(&mut self, val: Value, typ: &Typ) -> Value {
		let sym = match typ {
			Typ::Array(_) => runtime::ARRAY_SHARE,
			Typ::Map(..) => runtime::MAP_SHARE,
			_ => return val,
		};
		let func = self.import_fn(sym, &[self.int], Some(self.int));
		let call = self.b.ins().call(func, &[val]);
		self.b.inst_results(call)[0]
	}

	// Clone the buffer before a write if it's shared.
	pub(super) fn cow_array(&mut self, header: Value, elem: &Typ) {
		let stride = self.elem_stride(elem);
		let size = self.b.ins().iconst(self.int, stride);
		let func = self.import_fn(runtime::ARRAY_COW, &[self.int, self.int], None);
		self.b.ins().call(func, &[header, size]);
	}

	// Evaluate an array-typed operand.
	pub(super) fn array_operand(&mut self, collection: &Spanned<Expr>, what: &str) -> Result<TypedVal, Diagnostic> {
		let (ptr, typ) = self.expr(collection)?;
		match typ {
			Typ::Array(_) | Typ::FixedArray(..) => Ok((ptr, typ)),
			_ => Err(
				Diagnostic::new(format!("cannot {what} {typ}"), collection.1.into_range()).with_label("not an array"),
			),
		}
	}

	// (data pointer, length) for an array.
	pub(super) fn array_parts(&mut self, val: Value, typ: &Typ) -> (Value, Value) {
		match typ {
			Typ::FixedArray(_, n) => (val, self.b.ins().iconst(self.int, *n as i64)),
			_ => (self.array_data(val), self.array_len(val)),
		}
	}

	pub(super) fn int_value(&mut self, e: &Spanned<Expr>, what: &str) -> Result<Value, Diagnostic> {
		let (v, t) = self.expr(e)?;
		if !matches!(t, Typ::Int(_)) {
			return Err(
				Diagnostic::new(format!("{what} must be Int, got {t}"), e.1.into_range()).with_label("not an Int"),
			);
		}
		Ok(v)
	}

	// Bounds-check `idx` and return the element address.
	pub(super) fn elem_addr(&mut self, data: Value, len: Value, elem: &Typ, idx: Value) -> Value {
		let oob = self.b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, idx, len);

		let panic_block = self.b.create_block();
		let ok_block = self.b.create_block();
		self.b.ins().brif(oob, panic_block, &[], ok_block, &[]);
		self.b.seal_block(panic_block);
		self.b.seal_block(ok_block);

		self.b.switch_to_block(panic_block);
		let func = self.import_fn(runtime::PANIC_OOB, &[self.int, self.int], None);
		self.b.ins().call(func, &[idx, len]);
		self.b.ins().trap(TrapCode::HEAP_OUT_OF_BOUNDS);

		self.b.switch_to_block(ok_block);
		let stride = self.elem_stride(elem);
		let off = self.b.ins().imul_imm(idx, stride);
		self.b.ins().iadd(data, off)
	}

	pub(super) fn load_index(&mut self, data: Value, len: Value, elem: &Typ, idx: Value) -> Value {
		let addr = self.elem_addr(data, len, elem, idx);
		self.load_elem(addr, 0, elem)
	}

	pub(super) fn store_index(&mut self, data: Value, len: Value, elem: &Typ, idx: Value, val: Value) {
		let addr = self.elem_addr(data, len, elem, idx);
		self.store_elem(addr, 0, elem, val);
	}

	// An element's in-memory type.
	fn elem_mem(&self, elem: &Typ) -> Typ {
		match elem {
			Typ::Enum(_) => self.variants_of(elem).first().and_then(|v| v.backing.clone()),
			_ => None,
		}
		.unwrap_or_else(|| elem.clone())
	}

	pub(super) fn elem_stride(&self, elem: &Typ) -> i64 {
		elem_size(&self.elem_mem(elem))
	}

	pub(super) fn load_elem(&mut self, addr: Value, off: i32, elem: &Typ) -> Value {
		let mem = self.elem_mem(elem);
		let v = self.b.ins().load(cl_type(&mem, self.int), MemFlags::new(), addr, off);
		self.intcast(v, cl_type(elem, self.int), matches!(mem, Typ::Int(_)))
	}

	pub(super) fn store_elem(&mut self, addr: Value, off: i32, elem: &Typ, val: Value) {
		let mem = self.elem_mem(elem);
		let val = self.intcast(val, cl_type(&mem, self.int), matches!(mem, Typ::Int(_)));
		self.b.ins().store(MemFlags::new(), val, addr, off);
	}

	// nth element of a raw data pointer.
	pub(super) fn load_nth(&mut self, data: Value, idx: Value, elem: &Typ) -> Value {
		let stride = self.elem_stride(elem);
		let off = self.b.ins().imul_imm(idx, stride);
		let addr = self.b.ins().iadd(data, off);
		self.load_elem(addr, 0, elem)
	}
}
