//! Marshalling Oi structs to/from C layout.

use cranelift::prelude::*;
use cranelift_module::Module;

use super::{FieldDef, Translator, Typ, TypedVal, c_layout, check_c_sig, cl_type, display_name, is_c_struct};
use crate::ast::{Expr, Span, Spanned, TypeExpr};
use crate::diagnostics::Diagnostic;

impl<M: Module> Translator<'_, M> {
	pub(super) fn require_unsafe(&self, what: &str, span: Span) -> Result<(), Diagnostic> {
		match self.unsafely {
			0 => Err(Diagnostic::new(format!("`{what}` needs `unsafe`"), span.into_range())
				.with_label("no `unsafe` in scope")),
			_ => Ok(()),
		}
	}

	// Copy C struct in/out of foreign memory, based on `read`.
	pub(super) fn ptr_copy(
		&mut self,
		read: bool,
		recv: Option<Value>,
		type_args: &[Spanned<TypeExpr>],
		args: &[Spanned<Expr>],
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
		self.require_unsafe(if read { "ptr.read" } else { "ptr.write" }, span)?;
		match (read, recv, type_args, args) {
			(true, Some(c), [(te, ts)], []) => {
				let typ = self.types().resolve(te, *ts)?;
				let fields = self.c_fields(&typ, *ts)?;
				let oi = self.struct_slot(&fields)?;
				self.copy_fields(oi, c, 0, &fields, false);
				Ok((oi, typ))
			}
			(false, Some(c), [], [value]) => {
				let (oi, typ) = self.expr(value)?;
				let fields = self.c_fields(&typ, value.1)?;
				self.copy_fields(oi, c, 0, &fields, true);
				Ok(self.unit_value())
			}
			_ => {
				let usage = if read { "p.read[T]()" } else { "p.write(v)" };
				Err(
					Diagnostic::new("this copies a `@c` struct through a `ptr`", span.into_range())
						.with_label(format!("write `{usage}`")),
				)
			}
		}
	}

	// A `ptr` cast through a fn-type alias.
	pub(super) fn cast_fn_ptr(
		&mut self,
		name: &str,
		args: &[Spanned<Expr>],
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
		self.require_unsafe(&format!("{} cast", display_name(name)), span)?;
		let typ = self.types().resolve(&TypeExpr::Name(name.to_string()), span)?;
		let (sig, bare) = match &typ {
			Typ::Annotated(_, inner) => (&**inner, true),
			t => (t, false),
		};
		let (Typ::Fn(params, ret), [arg]) = (sig, args) else {
			let msg = format!("`{}` casts a single `ptr`", display_name(name));
			return Err(Diagnostic::new(msg, span.into_range()).with_label("expected one argument"));
		};
		check_c_sig(display_name(name), params, ret, span)?;
		if !bare && let Some(t) = params.iter().find(|t| matches!(t, Typ::Fn(..))) {
			let msg = format!("`{}` can't take a fn pointer", display_name(name));
			return Err(Diagnostic::new(msg, span.into_range()).with_label(format!("`{t}` would cross as a cell")));
		}
		let want = self.types().resolve(&TypeExpr::Name("core::ptr".into()), span)?;
		let addr = self.check_typed(arg, &want, "not a `ptr`")?;
		let val = if bare { addr } else { self.fn_cell(addr) };
		Ok((val, typ))
	}

	// Box a bare fn pointer as an Oi fn value.
	pub(crate) fn fn_cell(&mut self, addr: Value) -> Value {
		let cell = self.call_alloc_bytes(8);
		self.b.ins().store(MemFlags::new(), addr, cell, 0);
		cell
	}

	fn c_fields(&self, typ: &Typ, span: Span) -> Result<Vec<FieldDef>, Diagnostic> {
		match typ {
			Typ::Struct(name, fields) if is_c_struct(self.annotations, name) => Ok(fields.clone()),
			_ => Err(Diagnostic::new(format!("`{typ}` has no C layout"), span.into_range())
				.with_label("only a `@c` struct crosses as a struct")),
		}
	}

	// Copy each field between its Oi slot and its C offset.
	fn copy_fields(&mut self, oi: Value, c: Value, at: i32, fields: &[FieldDef], to_c: bool) {
		let (anns, mem) = (self.annotations, MemFlags::new());
		let offsets = c_layout(fields, &|n: &str| is_c_struct(anns, n)).expect("validated").offsets;
		for ((i, f), off) in fields.iter().enumerate().zip(offsets) {
			let (slot, off) = ((i * 8) as i32, at + off as i32);
			match f.typ.newtype().unwrap_or(&f.typ) {
				Typ::Struct(_, inner) => {
					// nested structs are inline in C, behind a pointer in Oi
					let child = self.b.ins().load(self.int, mem, oi, slot);
					self.copy_fields(child, c, off, inner, to_c);
				}
				Typ::Bool if to_c => {
					// bool is a byte in C but a word in Oi
					let v = self.b.ins().load(self.int, mem, oi, slot);
					self.b.ins().istore8(mem, v, c, off);
				}
				Typ::Bool => {
					let v = self.b.ins().uload8(self.int, mem, c, off);
					self.b.ins().store(mem, v, oi, slot);
				}
				Typ::FixedArray(e, n) => {
					let at = self.b.ins().iadd_imm(c, off as i64);
					match to_c {
						true => {
							let buf = self.b.ins().load(self.int, mem, oi, slot);
							self.fixed_move(at, buf, e, *n);
						}
						false => {
							let buf = self.fixed_copy(at, e, *n);
							self.b.ins().store(mem, buf, oi, slot);
						}
					}
				}
				typ => {
					let (src, so, dst, doff) = if to_c { (oi, slot, c, off) } else { (c, off, oi, slot) };
					let v = self.b.ins().load(cl_type(typ, self.int), mem, src, so);
					self.b.ins().store(mem, v, dst, doff);
				}
			}
		}
	}
}
