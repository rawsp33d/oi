use crate::compiler::expand;

use super::*;

impl<'a> Translator<'a> {
	// Lower a quote. Register its template and build the Ast it produces at runtime.
	pub(super) fn quote(&mut self, stmts: &[Spanned<Expr>], span: Span) -> Result<TypedVal, Diagnostic> {
		let (tpl, slots) = expand::register(stmts, span)?;
		let mut ptrs = Vec::with_capacity(slots.len());
		for slot in &slots {
			let ptr = match slot {
				expand::Slot::Name(name) => {
					let Some(local) = self.vars.get(name).cloned() else {
						return Err(Diagnostic::new(
							format!("`%{name}` refers to no binding in scope"),
							span.into_range(),
						)
						.with_label("not found in scope"));
					};
					let val = self.read_local(&local);
					self.lift_unquote(val, &local.typ, span)?
				}
				expand::Slot::Expr(e) => {
					let (val, typ) = self.expr(e)?;
					self.lift_unquote(val, &typ, span)?
				}
				expand::Slot::Splat(e) => {
					let (val, typ) = self.expr(e)?;
					if !matches!(&typ, Typ::Array(inner) if **inner == Typ::Ast) {
						return Err(Diagnostic::new(
							format!("can't spread a `{typ}`, expected `[]Ast`"),
							e.1.into_range(),
						)
						.with_label("not []Ast"));
					}
					// the header pointer itself: rt_quote reads the elements
					val
				}
			};
			ptrs.push(ptr);
		}
		let slot = if ptrs.is_empty() {
			self.b.ins().iconst(self.int, 0)
		} else {
			let slot = self.stack_slot((ptrs.len() * 8) as u32);
			for (i, v) in ptrs.iter().enumerate() {
				self.b.ins().store(MemFlags::new(), *v, slot, (i * 8) as i32);
			}
			slot
		};
		let len = self.b.ins().iconst(self.int, ptrs.len() as i64);
		let idxv = self.b.ins().iconst(self.int, tpl as i64);
		let func = self.import_fn(expand::RT_QUOTE, &[self.int; 3], Some(self.int));
		let call = self.b.ins().call(func, &[idxv, slot, len]);
		Ok((self.b.inst_results(call)[0], Typ::Ast))
	}

	// Lift an unquoted value into an Ast pointer, ready to splice into a template.
	fn lift_unquote(&mut self, val: Value, typ: &Typ, span: Span) -> Result<Value, Diagnostic> {
		match typ {
			Typ::Ast => Ok(val),
			Typ::Int(_) => {
				let v64 = self.intcast(val, types::I64, true);
				let func = self.import_fn(expand::RT_AST_INT, &[types::I64], Some(self.int));
				let call = self.b.ins().call(func, &[v64]);
				Ok(self.b.inst_results(call)[0])
			}
			other => Err(
				Diagnostic::new(format!("can't unquote a `{other}` yet"), span.into_range())
					.with_label("unsupported unquote type"),
			),
		}
	}

	// Call the runtime panic path with `msg` and mark the current block unreachable.
	fn abort(&mut self, msg: Value) -> TypedVal {
		let func = self.import_fn(runtime::PANIC, &[self.int], None);
		self.b.ins().call(func, &[msg]);
		self.b.ins().trap(TrapCode::HEAP_OUT_OF_BOUNDS);

		// unreachable paths
		let dead = self.b.create_block();
		self.b.seal_block(dead);
		self.b.switch_to_block(dead);
		self.unit_value()
	}

	// The optional message argument for the aborting macros.
	fn msg_arg(&mut self, name: &str, arg: Option<&Spanned<Expr>>, default: &str) -> Result<Value, Diagnostic> {
		let Some(arg) = arg else {
			return Ok(self.str_const(default));
		};
		match self.expr(arg)? {
			(val, Typ::Str) => Ok(val),
			(_, typ) => Err(
				Diagnostic::new(format!("`{name}!` message must be Str, got {typ}"), arg.1.into_range())
					.with_label("not a Str"),
			),
		}
	}

	pub(super) fn macro_call(
		&mut self,
		name: &str,
		args: &[Spanned<Expr>],
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
		let (min, max) = match name {
			"dbg" => (1, 1),
			"assert" => (1, 2),
			"panic" | "unreachable" => (0, 1),
			"todo" => (0, 0),
			_ => {
				return Err(
					Diagnostic::new(format!("no macro named `{name}!`"), span.into_range()).with_label("unknown macro")
				);
			}
		};
		if !(min..=max).contains(&args.len()) {
			let want = match (min, max) {
				(1, 1) => "1 argument".into(),
				(a, b) if a == b => format!("{a} arguments"),
				(a, b) => format!("{a} or {b} arguments"),
			};
			return Err(
				Diagnostic::new(format!("`{name}!` takes {want}, got {}", args.len()), span.into_range())
					.with_label("wrong number of arguments"),
			);
		}

		match name {
			"dbg" => {
				let (val, typ) = self.expr(&args[0])?;
				let (file, line, snippet) = self.map.locate_span(args[0].1.into_range());
				self.write_lit(&format!("[{file}:{line}] {snippet} = "), runtime::Sink::Err);
				self.emit_print(val, &typ, false, runtime::Sink::Err);
				self.write_lit("\n", runtime::Sink::Err);
				Ok((val, typ))
			}

			"assert" => {
				let (cond, typ) = self.expr(&args[0])?;
				if typ != Typ::Bool {
					return Err(Diagnostic::new(
						format!("`assert!` condition must be Bool, got {typ}"),
						args[0].1.into_range(),
					)
					.with_label("not a Bool"));
				}
				// the failure message defaults to the condition's source
				let snippet = self.map.locate_span(args[0].1.into_range()).2;
				let msg = self.msg_arg(name, args.get(1), snippet)?;

				let fail_block = self.b.create_block();
				let ok_block = self.b.create_block();
				self.b.ins().brif(cond, ok_block, &[], fail_block, &[]);
				self.b.seal_block(fail_block);
				self.b.seal_block(ok_block);

				self.b.switch_to_block(fail_block);
				let func = self.import_fn(runtime::ASSERT_FAIL, &[self.int], None);
				self.b.ins().call(func, &[msg]);
				self.b.ins().trap(TrapCode::HEAP_OUT_OF_BOUNDS);

				self.b.switch_to_block(ok_block);
				Ok(self.unit_value())
			}

			_ => {
				let default = match name {
					"panic" => "panicked",
					"todo" => "not yet implemented",
					_ => "entered unreachable code",
				};
				let msg = self.msg_arg(name, args.first(), default)?;
				Ok(self.abort(msg))
			}
		}
	}
}
