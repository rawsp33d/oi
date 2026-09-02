//! Marshalling Oi structs to/from C layout.

use cranelift::prelude::*;
use cranelift_module::Module;

use super::{FieldDef, Translator, Typ, TypedVal, c_layout, cl_type, is_c_struct};
use crate::ast::{Expr, Span, Spanned, TypeExpr};
use crate::diagnostics::Diagnostic;

impl<M: Module> Translator<'_, M> {
	// Copy C struct in/out of foreign memory, based on `read`.
	pub(super) fn ptr_copy(
		&mut self,
		read: bool,
		recv: Option<Value>,
		type_args: &[Spanned<TypeExpr>],
		args: &[Spanned<Expr>],
		span: Span,
	) -> Result<TypedVal, Diagnostic> {
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
				// nested structs are inline in C, behind a pointer in Oi
				Typ::Struct(_, inner) => {
					let child = self.b.ins().load(self.int, mem, oi, slot);
					self.copy_fields(child, c, off, inner, to_c);
				}
				// bool is a byte in C but a word in Oi
				Typ::Bool if to_c => {
					let v = self.b.ins().load(self.int, mem, oi, slot);
					self.b.ins().istore8(mem, v, c, off);
				}
				Typ::Bool => {
					let v = self.b.ins().uload8(self.int, mem, c, off);
					self.b.ins().store(mem, v, oi, slot);
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
