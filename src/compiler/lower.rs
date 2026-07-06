use std::collections::{HashMap, HashSet};
use std::ops::Range;

use cranelift::codegen;
use cranelift::codegen::ir::immediates::{Ieee16, Ieee128};
use cranelift::codegen::ir::{StackSlotData, StackSlotKind};
use cranelift::prelude::*;
use cranelift_jit::JITModule;
use cranelift_module::{DataDescription, Linkage, Module};

use super::{
	FieldDef, FnSig, Local, LoopFrame, Op, Typ, TypeCtx, VariantInfo, cl_int_for_width, cl_type, elem_size, enum_boxed,
	enum_slots,
};
use crate::ast::{Expr, MatchArm, Pattern, Span, Spanned, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::runtime;

mod control;
mod expr;
mod print;
mod stmt;

pub(super) struct Translator<'a> {
	pub int: types::Type,
	pub b: FunctionBuilder<'a>,
	pub vars: HashMap<String, Local>,
	pub module: &'a mut JITModule,
	pub funcs: &'a HashMap<String, FnSig>,
	pub structs: &'a HashMap<String, Vec<FieldDef>>,
	pub enums: &'a HashMap<String, Vec<VariantInfo>>,
	pub aliases: &'a HashMap<String, TypeExpr>,
	pub string_idx: &'a mut usize,
	pub atoms: &'a mut HashSet<String>,
	pub ret: Option<(Typ, Span)>,
	pub loops: Vec<LoopFrame>,
	pub self_type: Option<String>,
}

// A statement that writes through an existing, mutable binding.
#[derive(Clone, Copy)]
enum Mutation {
	Assign,      // `x = v`
	IndexAssign, // `x[i] = v`
	Append,      // `x << v`
	FieldAssign, // `x.f = v`
}

impl<'a> Translator<'a> {
	// The named types in scope, bundled for resolving type annotations.
	fn types(&self) -> TypeCtx<'a> {
		TypeCtx {
			structs: self.structs,
			enums: self.enums,
			aliases: self.aliases,
		}
	}

	// Look up the binding that a mutation targets.
	fn mutable_local(&self, name: &str, span: Range<usize>, op: Mutation) -> Result<Local, Diagnostic> {
		// how the mutation reads in errors
		// (verb, verb when immutable, noun for the `mut` hint, suggest `:=`?)
		let (verb, immutable_verb, allow, suggest_declare) = match op {
			Mutation::Assign => ("assign to", "assign to", "assignment", true),
			Mutation::IndexAssign => ("assign to", "assign to element of", "assignment", true),
			Mutation::Append => ("append to", "append to", "append", false),
			Mutation::FieldAssign => ("assign field of", "assign field of", "field assignment", false),
		};
		let local = self.vars.get(name).cloned().ok_or_else(|| {
			let d = Diagnostic::new(format!("cannot {verb} undefined variable `{name}`"), span.clone())
				.with_label("not found in scope");
			if suggest_declare {
				d.with_note(format!("declare it first with `{name} := ...`"))
			} else {
				d
			}
		})?;
		if !local.mutable {
			return Err(
				Diagnostic::new(format!("cannot {immutable_verb} immutable `{name}`"), span)
					.with_label("declared without `mut`")
					.with_note(format!("use `mut {name} := ...` to allow {allow}")),
			);
		}
		Ok(local)
	}

	// Widen/narrow integers.
	// Sign-extend `val` to i64, clamp to `[low, hi]`.
	fn clamp_to_width(
		&mut self,
		val: Value,
		extend_signed: bool,
		low: Option<(i64, bool)>,
		hi: i64,
		hi_unsigned: bool,
		target_cl: types::Type,
	) -> Value {
		let src_cl = self.b.func.dfg.value_type(val);
		let v64 = if src_cl == types::I64 {
			val
		} else if extend_signed {
			self.b.ins().sextend(types::I64, val)
		} else {
			self.b.ins().uextend(types::I64, val)
		};
		let v64 = match low {
			Some((low, lo_unsigned)) => {
				let lo_c = self.b.ins().iconst(types::I64, low);
				let cc = if lo_unsigned {
					IntCC::UnsignedLessThan
				} else {
					IntCC::SignedLessThan
				};
				let lt = self.b.ins().icmp(cc, v64, lo_c);
				self.b.ins().select(lt, lo_c, v64)
			}
			None => v64,
		};
		let hi_c = self.b.ins().iconst(types::I64, hi);
		let cc = if hi_unsigned {
			IntCC::UnsignedGreaterThan
		} else {
			IntCC::SignedGreaterThan
		};
		let gt = self.b.ins().icmp(cc, v64, hi_c);
		let v64 = self.b.ins().select(gt, hi_c, v64);
		if target_cl == types::I64 {
			v64
		} else {
			self.b.ins().ireduce(target_cl, v64)
		}
	}

	// Dispatch a call to a compiler builtin.
	fn builtin_call(
		&mut self,
		name: &str,
		args: &[Spanned<Expr>],
		span: Span,
	) -> Result<Option<(Value, Typ)>, Diagnostic> {
		match name {
			"print" | "write" | "eprint" | "ewrite" => {
				if args.is_empty() {
					return Err(
						Diagnostic::new(format!("`{name}` takes at least 1 argument"), span.into_range())
							.with_label("missing argument"),
					);
				}
				let stderr = matches!(name, "eprint" | "ewrite");
				let newline = matches!(name, "print" | "eprint");
				for (i, arg) in args.iter().enumerate() {
					if i > 0 {
						self.write_lit(" ", stderr);
					}
					let (val, typ) = self.expr(arg)?;
					self.emit_print(val, &typ, false, stderr);
				}
				if newline {
					self.write_lit("\n", stderr);
				}
				Ok(Some((self.b.ins().iconst(self.int, 0), Typ::Tuple(vec![]))))
			}

			// TODO: migrate to `assert!` macro once we have macros
			"assert" => {
				if args.is_empty() || args.len() > 2 {
					return Err(Diagnostic::new(
						format!("`assert` takes 1 or 2 arguments, got {}", args.len()),
						span.into_range(),
					)
					.with_label("wrong number of arguments"));
				}
				let (cond, cond_typ) = self.expr(&args[0])?;
				if cond_typ != Typ::Bool {
					return Err(Diagnostic::new(
						format!("`assert` condition must be Bool, got {cond_typ}"),
						args[0].1.into_range(),
					)
					.with_label("not a Bool"));
				}
				let msg = if args.len() == 2 {
					let (msg_val, msg_typ) = self.expr(&args[1])?;
					if msg_typ != Typ::Str {
						return Err(Diagnostic::new(
							format!("`assert` message must be Str, got {msg_typ}"),
							args[1].1.into_range(),
						)
						.with_label("not a Str"));
					}
					msg_val
				} else {
					self.str_const("assertion failed")
				};

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
				Ok(Some((cond, Typ::Bool)))
			}

			_ => self.cast_call(name, args, span),
		}
	}

	// A numeric cast builtin.
	fn cast_call(
		&mut self,
		name: &str,
		args: &[Spanned<Expr>],
		span: Span,
	) -> Result<Option<(Value, Typ)>, Diagnostic> {
		// `int` and `float` are aliases for the default-width casts
		let name = match name {
			"int" => "i32",
			"float" => "f64",
			other => other,
		};

		if matches!(name, "isize" | "usize") {
			let signed = name == "isize";
			let (val, typ) = self.cast_operand(name, args, span)?;
			let out = match (&typ, signed) {
				(Typ::ISize, true) | (Typ::USize, false) => val,
				// isize -> usize: clamp negative to 0
				(Typ::ISize, false) => {
					let zero = self.b.ins().iconst(self.int, 0);
					let lt = self.b.ins().icmp(IntCC::SignedLessThan, val, zero);
					self.b.ins().select(lt, zero, val)
				}
				// usize -> isize: saturate at isize::MAX
				(Typ::USize, true) => {
					let max_v = self.b.ins().iconst(self.int, i64::MAX);
					let gt = self.b.ins().icmp(IntCC::UnsignedGreaterThan, val, max_v);
					self.b.ins().select(gt, max_v, val)
				}
				// int -> isize: sign-extend
				(Typ::Int(_), true) => {
					let src_cl = cl_type(&typ, self.int);
					if src_cl == self.int {
						val
					} else {
						self.b.ins().sextend(self.int, val)
					}
				}
				// uint -> usize: zero-extend
				(Typ::UInt(_), false) => {
					let src_cl = cl_type(&typ, self.int);
					if src_cl == self.int {
						val
					} else {
						self.b.ins().uextend(self.int, val)
					}
				}
				// int -> usize: sign-extend then clamp negative to 0
				(Typ::Int(_), false) => {
					let src_cl = cl_type(&typ, self.int);
					let v = if src_cl == self.int {
						val
					} else {
						self.b.ins().sextend(self.int, val)
					};
					let zero = self.b.ins().iconst(self.int, 0);
					let lt = self.b.ins().icmp(IntCC::SignedLessThan, v, zero);
					self.b.ins().select(lt, zero, v)
				}
				// uint -> isize: zero-extend then saturate at isize::MAX
				(Typ::UInt(_), true) => {
					let src_cl = cl_type(&typ, self.int);
					let v = if src_cl == self.int {
						val
					} else {
						self.b.ins().uextend(self.int, val)
					};
					let max_v = self.b.ins().iconst(self.int, i64::MAX);
					let gt = self.b.ins().icmp(IntCC::UnsignedGreaterThan, v, max_v);
					self.b.ins().select(gt, max_v, v)
				}
				_ => {
					return Err(
						Diagnostic::new(format!("cannot cast {typ} to {name}"), args[0].1.into_range())
							.with_label("not an integer"),
					);
				}
			};
			let out_typ = if signed { Typ::ISize } else { Typ::USize };
			return Ok(Some((out, out_typ)));
		}

		if let Some(target) = int_cast_width('i', name) {
			let (val, typ) = self.cast_operand(name, args, span)?;
			let target_cl = cl_type(&Typ::Int(target), self.int);
			let out = match &typ {
				Typ::Int(w) if *w == target => val,
				Typ::Int(_) => self.clamp_to_width(
					val,
					true,
					Some((int_min(target), false)),
					int_max(target),
					false,
					target_cl,
				),
				Typ::Enum(enum_name) => {
					let tag = self.enum_tag(enum_name, val);
					if target_cl == types::I64 {
						tag
					} else {
						self.b.ins().ireduce(target_cl, tag)
					}
				}
				_ => {
					return Err(
						Diagnostic::new(format!("cannot cast {typ} to i{target}"), args[0].1.into_range())
							.with_label("not an integer"),
					);
				}
			};
			return Ok(Some((out, Typ::Int(target))));
		}

		if let Some(target) = int_cast_width('u', name) {
			let (val, typ) = self.cast_operand(name, args, span)?;
			let target_cl = cl_type(&Typ::UInt(target), self.int);
			let out = match &typ {
				Typ::UInt(w) if *w == target => val,
				Typ::UInt(_) => self.clamp_to_width(val, false, None, uint_max(target), true, target_cl),
				Typ::Int(_) => self.clamp_to_width(val, true, Some((0, false)), uint_max(target), true, target_cl),
				_ => {
					return Err(
						Diagnostic::new(format!("cannot cast {typ} to u{target}"), args[0].1.into_range())
							.with_label("not an integer"),
					);
				}
			};
			return Ok(Some((out, Typ::UInt(target))));
		}

		if matches!(name, "f16" | "f32" | "f64" | "f128") {
			let target: u16 = match name {
				"f16" => 16,
				"f32" => 32,
				"f128" => 128,
				_ => 64,
			};
			if args.len() != 1 {
				return Err(
					Diagnostic::new(format!("`{name}` cast takes exactly 1 argument"), span.into_range())
						.with_label("wrong number of arguments"),
				);
			}
			if target == 16 || target == 128 {
				return Err(Diagnostic::new(
					format!("f{target} casts are not yet supported by the JIT backend"),
					span.into_range(),
				)
				.with_label("not yet implemented"));
			}
			let (val, typ) = self.expr(&args[0])?;
			let target_cl = cl_type(&Typ::Float(target), self.int);
			let out = match &typ {
				Typ::Float(w) if *w == target => val,
				Typ::Float(_) if target == 64 => self.b.ins().fpromote(types::F64, val),
				Typ::Float(_) => self.b.ins().fdemote(types::F32, val),
				Typ::Int(_) => self.b.ins().fcvt_from_sint(target_cl, val),
				_ => {
					return Err(
						Diagnostic::new(format!("cannot cast {typ} to f{target}"), args[0].1.into_range())
							.with_label("not a number"),
					);
				}
			};
			return Ok(Some((out, Typ::Float(target))));
		}

		Ok(None)
	}

	// Evaluate the sole operand of a single-argument cast.
	// Errors on wrong arity.
	fn cast_operand(&mut self, name: &str, args: &[Spanned<Expr>], span: Span) -> Result<(Value, Typ), Diagnostic> {
		if args.len() != 1 {
			return Err(
				Diagnostic::new(format!("`{name}` cast takes exactly 1 argument"), span.into_range())
					.with_label("wrong number of arguments"),
			);
		}
		self.expr(&args[0])
	}

	fn str_const(&mut self, s: &str) -> Value {
		let mut bytes = s.as_bytes().to_vec();
		bytes.push(0);
		let name = format!("__str_{}", *self.string_idx);
		*self.string_idx += 1;
		let id = self.module.declare_data(&name, Linkage::Local, false, false).unwrap();
		let mut desc = DataDescription::new();
		desc.define(bytes.into_boxed_slice());
		self.module.define_data(id, &desc).unwrap();
		let gv = self.module.declare_data_in_func(id, self.b.func);
		self.b.ins().symbol_value(self.int, gv)
	}

	// Intern an atom name to a pointer-sized symbol.
	fn atom_const(&mut self, name: &str) -> Value {
		let sym = format!("__atom_{name}");
		if self.atoms.insert(name.to_string()) {
			let id = self.module.declare_data(&sym, Linkage::Local, false, false).unwrap();
			let mut bytes = format!(":{name}").into_bytes();
			bytes.push(0);
			let mut desc = DataDescription::new();
			desc.define(bytes.into_boxed_slice());
			self.module.define_data(id, &desc).unwrap();
		}
		let id = self.module.declare_data(&sym, Linkage::Local, false, false).unwrap();
		let gv = self.module.declare_data_in_func(id, self.b.func);
		self.b.ins().symbol_value(self.int, gv)
	}

	fn emit_eq(&mut self, a: Value, b: Value, typ: &Typ) -> Value {
		match typ {
			Typ::Float(_) => self.b.ins().fcmp(FloatCC::Equal, a, b),
			Typ::Str => {
				let func = self.import_fn(runtime::STR_EQ, &[self.int, self.int], Some(self.int));
				let call = self.b.ins().call(func, &[a, b]);
				self.b.inst_results(call)[0]
			}
			_ => self.b.ins().icmp(IntCC::Equal, a, b),
		}
	}

	// Sign-extend the low `w` bits of `val` within its Cranelift container.
	// A no-op for standard widths (8, 16, 32, 64).
	fn reduce_int(&mut self, val: Value, w: u16) -> Value {
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
	fn reduce_uint(&mut self, val: Value, w: u16) -> Value {
		let cl = cl_type(&Typ::UInt(w), self.int);
		if cl.bits() as u16 == w {
			return val;
		}
		let mask = ((1u64 << w) - 1) as i64;
		let mask_v = self.b.ins().iconst(cl, mask);
		self.b.ins().band(val, mask_v)
	}

	fn zero(&mut self, typ: &Typ) -> Value {
		match typ {
			Typ::Float(16) => self.b.ins().f16const(Ieee16::with_bits(0)),
			Typ::Float(32) => self.b.ins().f32const(0.0),
			Typ::Float(64) => self.b.ins().f64const(0.0),
			Typ::Float(128) => {
				let c = self.b.func.dfg.constants.insert(Ieee128::with_bits(0).into());
				self.b.ins().f128const(c)
			}
			Typ::Float(w) => panic!("unsupported float width f{w}"),
			Typ::Str => self.str_const(""),
			Typ::Atom => self.atom_const(""),
			Typ::Int(w) => self.b.ins().iconst(cl_type(&Typ::Int(*w), self.int), 0),
			Typ::UInt(w) => self.b.ins().iconst(cl_type(&Typ::UInt(*w), self.int), 0),
			Typ::Bool | Typ::ISize | Typ::USize => self.b.ins().iconst(self.int, 0),
			// default to first variant
			Typ::Enum(name) => {
				let disc = self.enums.get(name).and_then(|vs| vs.first()).map_or(0, |v| v.disc);
				self.make_enum(name, disc, &[])
			}
			Typ::Tuple(fields) if fields.is_empty() => self.b.ins().iconst(self.int, 0),
			Typ::Struct(_, fields) => {
				let fields = fields.clone();
				let size = (fields.len() * 8) as u32;
				let slot = self
					.b
					.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, size, 0));
				let ptr = self.b.ins().stack_addr(self.int, slot, 0);
				for (i, f) in fields.iter().enumerate() {
					let z = self.zero(&f.typ);
					self.b.ins().store(MemFlags::new(), z, ptr, (i * 8) as i32);
				}
				ptr
			}
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
				let stride = elem_size(&elem);
				let slot = self.b.create_sized_stack_slot(StackSlotData::new(
					StackSlotKind::ExplicitSlot,
					(*n as i64 * stride) as u32,
					0,
				));
				let ptr = self.b.ins().stack_addr(self.int, slot, 0);
				for i in 0..*n {
					let z = self.zero(&elem);
					self.b.ins().store(MemFlags::new(), z, ptr, (i as i64 * stride) as i32);
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
		}
	}

	// A numeric literal takes the binding's declared type directly.
	fn coerce_lit(&mut self, value: &Spanned<Expr>, target: &Typ) -> Result<Option<Value>, Diagnostic> {
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
			_ => return Ok(None),
		};
		Ok(Some(v))
	}

	// The tag of an enum value.
	fn enum_tag(&mut self, name: &str, val: Value) -> Value {
		if enum_boxed(self.enums.get(name).map(Vec::as_slice).unwrap_or(&[])) {
			self.b.ins().load(self.int, MemFlags::new(), val, 0)
		} else {
			val
		}
	}

	// Build a variant value.
	// A bare discriminant for fieldless enums, and a heap where that's not possible.
	fn make_enum(&mut self, name: &str, disc: i64, fields: &[Value]) -> Value {
		let slots = enum_slots(self.enums.get(name).map(Vec::as_slice).unwrap_or(&[]));
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
	fn enum_pattern(&self, pat: &Spanned<Expr>, enum_name: &str) -> Result<(i64, Vec<Bind>), Diagnostic> {
		let bad = |msg| Err(Diagnostic::new(msg, pat.1.into_range()).with_label("bad pattern"));
		let (variant, args): (&str, &[Spanned<Expr>]) = match &pat.0 {
			Expr::EnumShorthand { variant, args } => (variant, args),
			Expr::Atom(v) => (v, &[]),
			Expr::Field { tuple, field } if matches!(tuple.0, Expr::Ident(_)) => (field, &[]),
			_ => return bad(format!("`{enum_name}` is matched by its variants")),
		};
		let Some(v) = self.enums[enum_name].iter().find(|v| v.name == variant) else {
			return bad(format!("enum `{enum_name}` has no variant `{variant}`"));
		};
		let binds = field_binds(args.iter().zip(&v.payload), 8, 8)?;
		Ok((v.disc, binds))
	}

	fn range_pattern(
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
	fn construct_variant(
		&mut self,
		name: &str,
		variant: &str,
		args: &[Spanned<Expr>],
		span: Span,
	) -> Result<(Value, Typ), Diagnostic> {
		let v = self
			.enums
			.get(name)
			.and_then(|vs| vs.iter().find(|v| v.name == variant))
			.ok_or_else(|| {
				Diagnostic::new(format!("enum `{name}` has no variant `{variant}`"), span.into_range())
					.with_label("no such variant")
			})?;
		let (disc, payload) = (v.disc, v.payload.clone());
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
			let (fv, at) = self.check_expr(arg, ft)?;
			if at != *ft {
				let msg = format!("expected {ft}, got {at}");
				return Err(Diagnostic::new(msg, arg.1.into_range()).with_label("type mismatch"));
			}
			fields.push(fv);
		}
		let val = self.make_enum(name, disc, &fields);
		Ok((val, Typ::Enum(name.to_string())))
	}

	// Evaluate `value` against an expected type, resolving `.variant` shorthands and atoms via coercion.
	fn check_expr(&mut self, value: &Spanned<Expr>, target: &Typ) -> Result<(Value, Typ), Diagnostic> {
		if matches!(value.0, Expr::EnumShorthand { .. } | Expr::Atom(_))
			&& let Some(v) = self.coerce_lit(value, target)?
		{
			return Ok((v, target.clone()));
		}
		self.expr(value)
	}

	fn float_lit(&mut self, x: f64, w: u16, span: Span) -> Result<Value, Diagnostic> {
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

	fn struct_copy(&mut self, src: Value, fields: &[FieldDef]) -> Value {
		let size = (fields.len() * 8) as u32;
		let slot = self
			.b
			.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, size, 0));
		let dst = self.b.ins().stack_addr(self.int, slot, 0);
		for (i, f) in fields.iter().enumerate() {
			let cl = cl_type(&f.typ, self.int);
			let fv = self.b.ins().load(cl, MemFlags::new(), src, (i * 8) as i32);
			self.b.ins().store(MemFlags::new(), fv, dst, (i * 8) as i32);
		}
		dst
	}

	fn fixed_copy(&mut self, src: Value, elem: &Typ, n: usize) -> Value {
		let stride = elem_size(elem);
		let cl = cl_type(elem, self.int);
		let slot = self.b.create_sized_stack_slot(StackSlotData::new(
			StackSlotKind::ExplicitSlot,
			(n as i64 * stride) as u32,
			0,
		));
		let dst = self.b.ins().stack_addr(self.int, slot, 0);
		for i in 0..n {
			let off = (i as i64 * stride) as i32;
			let v = self.b.ins().load(cl, MemFlags::new(), src, off);
			self.b.ins().store(MemFlags::new(), v, dst, off);
		}
		dst
	}

	fn binop(&mut self, op: Op, l: &Spanned<Expr>, r: &Spanned<Expr>, span: Span) -> Result<(Value, Typ), Diagnostic> {
		let (lv, lt) = self.expr(l)?;
		let (rv, rt) = self.expr(r)?;

		// string concatenation
		if let (Op::Add, Typ::Str, Typ::Str) = (op, &lt, &rt) {
			return Ok((self.call_concat(lv, rv), Typ::Str));
		}

		#[derive(Clone, Copy)]
		enum NumKind {
			Int,
			UInt,
			Float,
		}
		// NOTE: might go with V-style int/float promotion eventually
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
		if let (Op::Mod, NumKind::Float) = (op, kind) {
			// TODO: cranelift has no float remainder
			return Err(
				Diagnostic::new("`%` is not yet supported on floats".to_string(), span.into_range())
					.with_label("only integer operands"),
			);
		}
		let b = self.b.ins();
		let out = match (op, kind) {
			(Op::Add, NumKind::Float) => b.fadd(lv, rv),
			(Op::Add, _) => b.iadd(lv, rv),
			(Op::Sub, NumKind::Float) => b.fsub(lv, rv),
			(Op::Sub, _) => b.isub(lv, rv),
			(Op::Mul, NumKind::Float) => b.fmul(lv, rv),
			(Op::Mul, _) => b.imul(lv, rv),
			(Op::Div, NumKind::Float) => b.fdiv(lv, rv),
			(Op::Div, NumKind::UInt) => b.udiv(lv, rv),
			(Op::Div, NumKind::Int) => b.sdiv(lv, rv),
			(Op::Mod, NumKind::Float) => unreachable!("float `%` rejected above"),
			(Op::Mod, NumKind::UInt) => b.urem(lv, rv),
			(Op::Mod, NumKind::Int) => b.srem(lv, rv),
		};
		// For non-standard widths, wrap the result back to the declared bit width.
		let out = match &lt {
			Typ::Int(w) if cl_type(&Typ::Int(*w), self.int).bits() as u16 != *w => self.reduce_int(out, *w),
			Typ::UInt(w) if cl_type(&Typ::UInt(*w), self.int).bits() as u16 != *w => self.reduce_uint(out, *w),
			_ => out,
		};
		Ok((out, lt))
	}

	fn cmp(
		&mut self,
		icc: IntCC,
		fcc: FloatCC,
		l: &Spanned<Expr>,
		r: &Spanned<Expr>,
		span: Span,
	) -> Result<(Value, Typ), Diagnostic> {
		// evaluate the typed/pinned side first so a `.variant` shorthand can borrow its enum type
		let ((lv, lt), (rv, rt)) = if let Expr::EnumShorthand { .. } = &l.0 {
			let (rv, rt) = self.expr(r)?;
			(self.check_expr(l, &rt)?, (rv, rt))
		} else {
			let (lv, lt) = self.expr(l)?;
			let rhs = self.check_expr(r, &lt)?;
			((lv, lt), rhs)
		};

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
			(Typ::Enum(a), Typ::Enum(b)) if a == b => {
				if enum_boxed(self.enums.get(a).map(Vec::as_slice).unwrap_or(&[])) {
					return Err(Diagnostic::new(
						format!("`{a}` has payloads, so `==` isn't supported yet"),
						span.into_range(),
					)
					.with_label("match on the variant instead"));
				}
				self.b.ins().icmp(icc, lv, rv)
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

	// Short-circuits. `&&` only evaluates the right side when the left is true, and `||` does the inverse.
	fn logical(&mut self, and: bool, l: &Spanned<Expr>, r: &Spanned<Expr>) -> Result<(Value, Typ), Diagnostic> {
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

	fn import_fn(&mut self, name: &str, params: &[types::Type], ret: Option<types::Type>) -> codegen::ir::FuncRef {
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

	// Emit a call to a resolved fn.
	fn call_sig(
		&mut self,
		name: &str,
		sig: FnSig,
		recv: Option<Value>,
		args: &[Spanned<Expr>],
		span: Span,
	) -> Result<(Value, Typ), Diagnostic> {
		let self_n = recv.is_some() as usize;
		if args.len() + self_n != sig.params.len() {
			return Err(Diagnostic::new(
				format!(
					"`{name}` expects {} argument(s), got {}",
					sig.params.len() - self_n,
					args.len()
				),
				span.into_range(),
			)
			.with_label("wrong number of arguments"));
		}
		let mut vals = Vec::with_capacity(args.len() + self_n);
		let mut expected = sig.params.iter();
		if let Some(recv) = recv {
			expected.next();
			vals.push(recv);
		}
		for arg in args {
			let (val, typ) = self.expr(arg)?;
			let want = expected.next().unwrap();
			if &typ != want {
				return Err(
					Diagnostic::new(format!("expected {want} argument, got {typ}"), arg.1.into_range())
						.with_label("wrong argument type"),
				);
			}
			vals.push(val);
		}
		let func = self.module.declare_func_in_func(sig.id, self.b.func);
		let call = self.b.ins().call(func, &vals);
		let ret_val = if matches!(sig.ret, Typ::Tuple(ref f) if f.is_empty()) {
			self.b.ins().iconst(self.int, 0)
		} else {
			self.b.inst_results(call)[0]
		};
		Ok((ret_val, sig.ret))
	}

	fn call_concat(&mut self, a: Value, b: Value) -> Value {
		let func = self.import_fn(runtime::STR_CONCAT, &[self.int, self.int], Some(self.int));
		let call = self.b.ins().call(func, &[a, b]);
		self.b.inst_results(call)[0]
	}

	fn call_alloc(&mut self, n: usize) -> Value {
		self.call_alloc_bytes((n * 8) as i64)
	}

	fn call_alloc_bytes(&mut self, bytes: i64) -> Value {
		let func = self.import_fn(runtime::ALLOC, &[self.int], Some(self.int));
		let size = self.b.ins().iconst(self.int, bytes);
		let call = self.b.ins().call(func, &[size]);
		self.b.inst_results(call)[0]
	}

	// array handle: { data @ 0, len @ 8, cap @ 16 }
	fn array_data(&mut self, header: Value) -> Value {
		self.b.ins().load(self.int, MemFlags::new(), header, 0)
	}
	fn array_len(&mut self, header: Value) -> Value {
		self.b.ins().load(self.int, MemFlags::new(), header, 8)
	}
	fn array_cap(&mut self, header: Value) -> Value {
		self.b.ins().load(self.int, MemFlags::new(), header, 16)
	}
	fn make_array(&mut self, data: Value, len: Value) -> Value {
		let header = self.call_alloc(3);
		self.b.ins().store(MemFlags::new(), data, header, 0);
		self.b.ins().store(MemFlags::new(), len, header, 8);
		self.b.ins().store(MemFlags::new(), len, header, 16);
		header
	}

	// Evaluate an array-typed operand, returning its value and type.
	fn array_operand(&mut self, collection: &Spanned<Expr>, what: &str) -> Result<(Value, Typ), Diagnostic> {
		let (ptr, typ) = self.expr(collection)?;
		match typ {
			Typ::Array(_) | Typ::FixedArray(..) => Ok((ptr, typ)),
			_ => Err(
				Diagnostic::new(format!("cannot {what} {typ}"), collection.1.into_range()).with_label("not an array"),
			),
		}
	}

	// (data pointer, length) for an array.
	fn array_parts(&mut self, val: Value, typ: &Typ) -> (Value, Value) {
		match typ {
			Typ::FixedArray(_, n) => (val, self.b.ins().iconst(self.int, *n as i64)),
			_ => (self.array_data(val), self.array_len(val)),
		}
	}

	fn int_value(&mut self, e: &Spanned<Expr>, what: &str) -> Result<Value, Diagnostic> {
		let (v, t) = self.expr(e)?;
		if !matches!(t, Typ::Int(_)) {
			return Err(
				Diagnostic::new(format!("{what} must be Int, got {t}"), e.1.into_range()).with_label("not an Int"),
			);
		}
		Ok(v)
	}

	// Bounds-check `idx` and return the element address.
	fn elem_addr(&mut self, data: Value, len: Value, elem: &Typ, idx: Value) -> Value {
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
		let off = self.b.ins().imul_imm(idx, elem_size(elem));
		self.b.ins().iadd(data, off)
	}

	fn load_index(&mut self, data: Value, len: Value, elem: &Typ, idx: Value) -> Value {
		let addr = self.elem_addr(data, len, elem, idx);
		self.b.ins().load(cl_type(elem, self.int), MemFlags::new(), addr, 0)
	}

	fn store_index(&mut self, data: Value, len: Value, elem: &Typ, idx: Value, val: Value) {
		let addr = self.elem_addr(data, len, elem, idx);
		self.b.ins().store(MemFlags::new(), val, addr, 0);
	}
}

// A destructured binding.
// `(name, type, offset)`
type Bind = (String, Typ, i32);

// Create `Bind`s from idents.
// `base` is the first offset, `stride` the step between fields.
fn field_binds<'a>(
	elems: impl Iterator<Item = (&'a Spanned<Expr>, &'a Typ)>,
	base: i32,
	stride: i32,
) -> Result<Vec<Bind>, Diagnostic> {
	elems
		.enumerate()
		.map(|(i, (e, t))| match &e.0 {
			Expr::Ident(n) => Ok((n.clone(), t.clone(), base + i as i32 * stride)),
			_ => Err(Diagnostic::new("patterns must bind names", e.1.into_range()).with_label("not a name")),
		})
		.collect()
}

// A struct pattern's field bindings.
fn struct_pattern(
	fdefs: &[FieldDef],
	pname: &str,
	sname: &str,
	entries: &[(Option<String>, Spanned<Expr>)],
	span: Span,
) -> Result<Vec<Bind>, Diagnostic> {
	if pname != sname {
		let msg = format!("pattern is `{pname}` but subject is `{sname}`");
		return Err(Diagnostic::new(msg, span.into_range()).with_label("type mismatch"));
	}
	entries
		.iter()
		.map(|(fname, e)| {
			let Expr::Ident(local) = &e.0 else {
				return Err(
					Diagnostic::new("struct patterns must bind names", e.1.into_range()).with_label("not a name")
				);
			};
			let field = fname.as_deref().unwrap_or(local);
			let idx = fdefs.iter().position(|f| f.name == field).ok_or_else(|| {
				Diagnostic::new(format!("struct `{sname}` has no field `{field}`"), e.1.into_range())
					.with_label("no such field")
			})?;
			Ok((local.clone(), fdefs[idx].typ.clone(), idx as i32 * 8))
		})
		.collect()
}

// The element type of an array.
fn array_elem(typ: &Typ) -> &Typ {
	match typ {
		Typ::Array(e) | Typ::FixedArray(e, _) => e,
		_ => unreachable!("not an array type"),
	}
}

// The width of `i{N}` and `i{N}` casts.
fn int_cast_width(prefix: char, name: &str) -> Option<u16> {
	name.strip_prefix(prefix)
		.and_then(|w| w.parse::<u16>().ok())
		.filter(|&w| w > 0 && w <= 64)
}

fn uint_max(width: u16) -> i64 {
	if width >= 64 {
		u64::MAX as i64
	} else {
		((1u64 << width) - 1) as i64
	}
}

fn int_min(width: u16) -> i64 {
	if width >= 64 { i64::MIN } else { -(1i64 << (width - 1)) }
}

fn int_max(width: u16) -> i64 {
	if width >= 64 {
		i64::MAX
	} else {
		(1i64 << (width - 1)) - 1
	}
}

fn unsigned_cc(icc: IntCC) -> IntCC {
	match icc {
		IntCC::SignedLessThan => IntCC::UnsignedLessThan,
		IntCC::SignedLessThanOrEqual => IntCC::UnsignedLessThanOrEqual,
		IntCC::SignedGreaterThan => IntCC::UnsignedGreaterThan,
		IntCC::SignedGreaterThanOrEqual => IntCC::UnsignedGreaterThanOrEqual,
		other => other,
	}
}
