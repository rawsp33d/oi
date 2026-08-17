use super::*;

impl<'a> Translator<'a> {
	pub fn expr(&mut self, expr: &Spanned<Expr>) -> Result<TypedVal, Diagnostic> {
		match &expr.0 {
			Expr::Int(n) => {
				if (i32::MIN as i64..=i32::MAX as i64).contains(n) {
					Ok((self.b.ins().iconst(types::I32, *n), Typ::Int(32)))
				} else {
					Ok((self.b.ins().iconst(types::I64, *n), Typ::Int(64)))
				}
			}
			Expr::Bool(v) => Ok((self.b.ins().iconst(self.int, *v as i64), Typ::Bool)),
			Expr::Float(x) => Ok((self.b.ins().f64const(*x), Typ::Float(64))),
			Expr::String(s) => Ok((self.str_const(s), Typ::Str)),
			Expr::Atom(name) => Ok((self.atom_const(name), Typ::Atom)),

			Expr::EnumShorthand { variant, .. } => Err(Diagnostic::new(
				format!("cannot infer the enum type of `.{variant}` here"),
				expr.1.into_range(),
			)
			.with_label("no enum type is expected in this position")
			.with_note(format!("qualify it, e.g. `Color.{variant}`"))),

			Expr::None => Err(
				Diagnostic::new("cannot infer the type of `none` here", expr.1.into_range())
					.with_label("no `?T` type is expected in this position")
					.with_note("qualify it (ex: `?int(none)`)"),
			),

			Expr::MutArg(_) => Err(
				Diagnostic::new("`mut` is only allowed on call arguments", expr.1.into_range())
					.with_label("not a call argument"),
			),

			Expr::OptionInit { inner: (te, span), arg } => {
				let inner_typ = self.types().resolve(te, *span)?;
				let val = if matches!(arg.0, Expr::None) {
					self.make_option(&inner_typ, None)
				} else {
					let fv = self.check_typed(arg, &inner_typ, "type mismatch")?;
					self.make_option(&inner_typ, Some(fv))
				};
				Ok((val, Typ::Option(Box::new(inner_typ))))
			}

			Expr::ResultInit { inner: (te, span), arg } => {
				let inner_typ = self.types().resolve(te, *span)?;
				let variants = result_variants(&inner_typ);
				let (fv, at) = self.check_expr(arg, &inner_typ)?;
				let disc = if at == inner_typ {
					0
				} else if at == Typ::Error {
					1
				} else {
					return Err(Diagnostic::new(
						format!("expected {inner_typ} or Error, got {at}"),
						arg.1.into_range(),
					)
					.with_label("type mismatch"));
				};
				let val = self.make_enum(&variants, disc, &[fv]);
				Ok((val, Typ::Result(Box::new(inner_typ))))
			}

			Expr::Ident(name) => match self.local(name, expr.1.into_range()) {
				Ok(local) => {
					let val = self.read_local(&local);
					Ok((val, local.typ))
				}
				Err(e) => match self.funcs.get(self.qualify(name).as_ref()).cloned() {
					Some(sig) => {
						let func_ref = self.module.declare_func_in_func(sig.id, self.b.func);
						let addr = self.b.ins().func_addr(self.int, func_ref);
						Ok((addr, Typ::Fn(sig.value_params(), Box::new(sig.ret))))
					}
					None => match self.consts.get(self.qualify(name).as_ref()).cloned() {
						Some(c) => self.expr(&c),
						None => Err(e),
					},
				},
			},

			Expr::Dollar => Ok(self.dollar()),

			Expr::Is {
				subject,
				trait_name,
				negated,
			} => {
				let Expr::Ident(name) = &subject.0 else {
					return Err(Diagnostic::new(
						"`is` takes a type name on the left",
						subject.1.into_range(),
					));
				};
				let typ = self.types().resolve(&TypeExpr::Name(name.clone()), subject.1)?;
				let holds = self.trait_impls.contains(&(typ.to_string(), trait_name.clone())) ^ negated;
				Ok((self.b.ins().iconst(self.int, holds as i64), Typ::Bool))
			}

			Expr::Negative(e) => {
				let (v, typ) = self.expr(e)?;
				let out = match typ {
					Typ::Int(_) => self.b.ins().ineg(v),
					Typ::Float(_) => self.b.ins().fneg(v),
					Typ::Struct(ref name, _) => match self.fill(name, "Neg", "neg", 1) {
						Some(sig) => return Ok(self.emit_call(&sig, &[v])),
						None => {
							return Err(Diagnostic::new(format!("cannot negate {typ}"), expr.1.into_range())
								.with_label(format!("claim `Neg` for `{name}`")));
						}
					},
					_ => {
						return Err(Diagnostic::new(format!("cannot negate {typ}"), expr.1.into_range())
							.with_label(format!("this is {typ}")));
					}
				};
				Ok((out, typ))
			}

			Expr::Binary(op, l, r) => match op {
				BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => self.binop(*op, l, r, expr.1),
				BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
					let (icc, fcc) = cmp_cc(*op);
					self.cmp(icc, fcc, l, r, expr.1)
				}
				BinOp::And => self.logical(true, l, r),
				BinOp::Or => self.logical(false, l, r),
				BinOp::In => self.in_op(l, r),
			},
			Expr::Not(e) => {
				let (v, typ) = self.expr(e)?;
				if typ != Typ::Bool {
					return Err(
						Diagnostic::new(format!("expected Bool, got {typ}"), expr.1.into_range())
							.with_label("`!` needs a Bool operand"),
					);
				}
				// a bool is always 0 or 1, so flipping the low bit negates it
				Ok((self.b.ins().bxor_imm(v, 1), Typ::Bool))
			}

			Expr::Call { name, type_args, args } => {
				let qn = self.qualify(name).to_string();
				if !type_args.is_empty() && !self.generic_fns.contains_key(&qn) {
					return Err(Diagnostic::new(format!("`{name}` is not generic"), expr.1.into_range())
						.with_label("unexpected type arguments"));
				}
				if let Some(local) = self.vars.get(name).cloned() {
					let callee = self.read_local(&local);
					return self.call_value(name, callee, &local.typ, args, None, expr.1);
				}
				match self.builtin_call(name, args, expr.1)? {
					Some(result) => Ok(result),
					None => match self.funcs.get(&qn).cloned() {
						Some(sig) => self.call_sig(name, sig, None, None, args, expr.1),
						None => match self.generic_fns.get(&qn).cloned() {
							Some(def) => self.call_generic(&qn, &def, type_args, args, None, expr.1),
							None if matches!(self.aliases.get(&qn), Some(TypeExpr::TupleStruct(..))) => {
								self.construct_tuple_struct(&qn, args, expr.1)
							}
							None => Err(
								Diagnostic::new(format!("undefined function `{name}`"), expr.1.into_range())
									.with_label("not defined"),
							),
						},
					},
				}
			}

			Expr::MethodCall { recv, method, args } => {
				// qualified access to an imported module's function
				if let Expr::Ident(m) = &recv.0
					&& !self.vars.contains_key(m)
					&& let Some(vis) = self.scope.visible.get(m)
				{
					// narrowed imports
					let target = match &vis.only {
						None => method,
						Some(only) => only.get(method).ok_or_else(|| {
							Diagnostic::new(format!("`{method}` is not part of `{m}`"), expr.1.into_range())
								.with_label("not in this import")
						})?,
					};
					let (module, target) = (vis.module.clone(), target.clone());
					return self.module_call(&module, &target, args, expr.1);
				}

				// enum payload
				if let Expr::Ident(name) = &recv.0
					&& !self.vars.contains_key(name)
					&& self.enums.contains_key(self.qualify(name).as_ref())
				{
					let name = self.qualify(name).to_string();
					return if method == "from" {
						self.enum_from(&name, args, expr.1)
					} else {
						self.construct_variant(&name, method, args, expr.1)
					};
				}

				// method call is static when `recv` names a struct
				let (sname, bound) = if let Expr::Ident(name) = &recv.0
					&& !self.vars.contains_key(name)
					&& (self.structs.contains_key(self.qualify(name).as_ref())
						|| matches!(
							self.aliases.get(self.qualify(name).as_ref()),
							Some(TypeExpr::TupleStruct(..))
						)) {
					(self.qualify(name).to_string(), None)
				} else {
					let (recv_val, recv_typ) = self.expr(recv)?;
					let recv_typ = self.peeled(&recv_typ);
					if method == "str"
						&& args.is_empty() && !matches!(recv_typ, Typ::Struct(..) | Typ::TupleStruct(..) | Typ::Enum(..))
					{
						return Ok((self.derived_str(recv_val, &recv_typ), Typ::Str));
					}
					if let Typ::Trait(tn) = &recv_typ {
						return self.dyn_call(recv_val, tn, method, args, expr.1);
					}

					// `Error` trait
					if recv_typ == Typ::Error {
						if method == "message" && args.is_empty() {
							return Ok((recv_val, Typ::Str));
						}
						return Err(
							Diagnostic::new(format!("`Error` has no method `{method}`"), expr.1.into_range())
								.with_label("no such method"),
						);
					}
					match &recv_typ {
						Typ::Struct(name, _) | Typ::TupleStruct(name, _) | Typ::Enum(name) => {
							(name.clone(), Some((recv_val, recv_typ)))
						}
						_ => {
							return Err(
								Diagnostic::new(format!("`{recv_typ}` has no methods"), recv.1.into_range())
									.with_label("methods are only defined on structs"),
							);
						}
					}
				};
				let recv_expr = bound.is_some().then(|| recv.as_ref());
				let key = format!("{sname}.{method}");
				if let Some(sig) = self.funcs.get(&key).cloned() {
					return self.call_sig(&key, sig, bound.map(|(v, _)| v), recv_expr, args, expr.1);
				}
				let gkey = format!("{}.{method}", sname.split('[').next().unwrap());
				if let Some(def) = self.generic_fns.get(&gkey).cloned() {
					return self.call_generic(&gkey, &def, &[], args, bound.zip(recv_expr), expr.1);
				}
				if method == "str"
					&& args.is_empty()
					&& let Some((v, t)) = bound
				{
					return Ok((self.derived_str(v, &t), Typ::Str));
				}
				Err(
					Diagnostic::new(format!("`{sname}` has no method `{method}`"), expr.1.into_range())
						.with_label("no such method"),
				)
			}

			// a tuple is a heap block of pointer-sized slots, one per field
			Expr::Tuple(elems) => {
				if elems.is_empty() {
					return Ok(self.unit_value());
				}
				let ptr = self.call_alloc(elems.len());
				let mut fields = Vec::with_capacity(elems.len());
				for (i, (name, value)) in elems.iter().enumerate() {
					let (val, typ) = self.expr(value)?;
					let val = self.copy_in(val, &typ);
					self.b.ins().store(MemFlags::new(), val, ptr, (i * 8) as i32);
					fields.push((name.clone(), typ));
				}
				Ok((ptr, Typ::Tuple(fields)))
			}

			Expr::Field { tuple, field } => {
				// access an imported module's items
				if let Expr::Ident(m) = &tuple.0
					&& !self.vars.contains_key(m)
					&& let Some(vis) = self.scope.visible.get(m)
				{
					let target = match &vis.only {
						None => field,
						Some(only) => only.get(field).ok_or_else(|| {
							Diagnostic::new(format!("`{field}` is not part of `{m}`"), expr.1.into_range())
								.with_label("not in this import")
						})?,
					};
					let module = vis.module.clone();
					let key = format!("{module}::{target}");
					let key = self.reexports.get(&key).cloned().unwrap_or(key);
					let (msg, label) = match self.consts.get(&key).cloned() {
						Some(c) if self.publics.contains(&key) => return self.expr(&c),
						Some(_) => (format!("`{field}` is private to module `{module}`"), "not public"),
						None if self.funcs.contains_key(&key) || self.generic_fns.contains_key(&key) => {
							(format!("`{field}` is a function, call it"), "add `()`")
						}
						None => (format!("module `{module}` has no const `{field}`"), "no such const"),
					};
					return Err(Diagnostic::new(msg, expr.1.into_range()).with_label(label));
				}

				// enum variants
				if let Expr::Ident(name) = &tuple.0
					&& !self.vars.contains_key(name)
					&& self.enums.contains_key(self.qualify(name).as_ref())
				{
					let name = self.qualify(name).to_string();
					return self.construct_variant(&name, field, &[], expr.1);
				}

				let (ptr, typ) = self.expr(tuple)?;
				let typ = self.peeled(&typ);

				if let Typ::Trait(tn) = &typ {
					return self.trait_field(ptr, tn, field, expr.1);
				}

				// arrays expose `.len` and numeric `.n` (sugar for `arr[n]`)
				if let Typ::Array(_) | Typ::FixedArray(..) = &typ {
					let elem = array_elem(&typ).clone();
					let (data, len) = self.array_parts(ptr, &typ);
					if field == "len" {
						let len = self.b.ins().ireduce(types::I32, len);
						return Ok((len, Typ::Int(32)));
					}
					return match field.parse::<i64>() {
						Ok(n) => {
							let idx = self.b.ins().iconst(self.int, n);
							Ok((self.load_index(data, len, &elem, idx), elem))
						}
						Err(_) => Err(
							Diagnostic::new(format!("arrays have no field `{field}`"), expr.1.into_range())
								.with_label("arrays only have `.len` and numeric indices"),
						),
					};
				}

				// structs are just fully-named tuples at the codegen level
				let typ = match typ {
					Typ::Struct(_, fields) => Typ::Tuple(fields.into_iter().map(|f| (Some(f.name), f.typ)).collect()),
					Typ::TupleStruct(_, fields) => Typ::Tuple(fields),
					other => other,
				};

				let fields = match &typ {
					Typ::Tuple(fields) => fields,
					_ => {
						return Err(
							Diagnostic::new(format!("cannot access a field of {typ}"), tuple.1.into_range())
								.with_label("not a tuple"),
						);
					}
				};
				let idx = match field.parse::<usize>() {
					Ok(i) if i < fields.len() => i,
					Ok(i) => {
						return Err(Diagnostic::new(
							format!("tuple index {i} out of range (len {})", fields.len()),
							expr.1.into_range(),
						)
						.with_label("no such field"));
					}
					Err(_) => fields
						.iter()
						.position(|(name, _)| name.as_deref() == Some(field.as_str()))
						.ok_or_else(|| {
							Diagnostic::new(format!("tuple has no field `{field}`"), expr.1.into_range())
								.with_label("no such field")
						})?,
				};
				let field_typ = fields[idx].1.clone();
				let cl = cl_type(&field_typ, self.int);
				let v = self.b.ins().load(cl, MemFlags::new(), ptr, (idx * 8) as i32);
				Ok((v, field_typ))
			}

			Expr::Array(elems) => self.array_lit(elems, None, expr.1),

			Expr::DotArray(None, elems) => self.fixed_infer(elems, expr.1),

			Expr::DotTuple(_) => Err(
				Diagnostic::new("cannot infer the tuple struct here", expr.1.into_range())
					.with_label("annotate the binding, or construct with `Name( ... )`"),
			),

			Expr::DotArray(Some((te, span)), elems) => {
				let elem = self.types().resolve(te, *span)?;
				if elems.is_empty() {
					return Err(
						Diagnostic::new("an exact array literal needs elements", expr.1.into_range())
							.with_label(format!("write `[]{elem}` for an empty dynamic array")),
					);
				}
				self.fixed_lit(elems, &elem, elems.len(), expr.1)
			}

			Expr::Index { collection, index } => {
				let (ptr, typ) = self.expr(collection)?;
				match &typ {
					Typ::Map(k, v) => {
						let (k, v) = (*k.clone(), *v.clone());
						let (tag, bits) = self.map_key(index, &k)?;
						let raw = self.call_map_get(ptr, tag, bits);
						Ok((self.unmap_bits(raw, &v), v))
					}
					Typ::Array(_) | Typ::FixedArray(..) => {
						let elem = array_elem(&typ).clone();
						let idx = self.int_value(index, "array index")?;
						let idx = self.b.ins().sextend(self.int, idx);
						let (data, len) = self.array_parts(ptr, &typ);
						Ok((self.load_index(data, len, &elem, idx), elem))
					}
					_ => Err(
						Diagnostic::new(format!("cannot index {typ}"), collection.1.into_range())
							.with_label("not indexable"),
					),
				}
			}

			Expr::Slice { collection, start, end } => {
				let (out, _, elem) = self.slice_copy(collection, start, end)?;
				let typ = Typ::Array(Box::new(elem));
				self.temp(out, &typ);
				Ok((out, typ))
			}

			Expr::If { cond, then, els } => match self.conditional(cond, then, els.as_deref(), None, expr.1)? {
				Some((v, t)) => Ok((v, t)),
				None => Err(Diagnostic::new("this `if` never produces a value", expr.1.into_range())
					.with_label("every branch returns, but a value is needed here")),
			},

			Expr::Match {
				subject,
				arms,
				else_body,
			} => match self.match_expr(subject, arms, else_body.as_deref(), None, expr.1)? {
				Some((v, t)) => Ok((v, t)),
				None => Err(
					Diagnostic::new("this `match` never produces a value", expr.1.into_range())
						.with_label("every arm returns, but a value is needed here"),
				),
			},

			Expr::Pipe { value, step } => self.pipe(value, step, expr.1),

			Expr::OrElse { value, body } => self.or_else(value, body, expr.1),
			Expr::Propagate(value) => self.propagate(value, expr.1),

			Expr::Loop { cond, body } => match self.loop_expr(cond.as_deref(), body)? {
				Some(vt) => Ok(vt),
				None => Err(
					Diagnostic::new("this `loop` never produces a value", expr.1.into_range())
						.with_label("an infinite loop with no `break` yields nothing"),
				),
			},

			Expr::For { pat, iter, body } => self.for_loop(pat, iter, body, expr.1),

			Expr::StructLit { name, fields } => self.struct_lit(name, fields, expr.1, None),

			Expr::Ref(inner) => {
				let (ptr, typ) = self.expr(inner)?;
				let Typ::Struct(name, fields) = &typ else {
					return Err(
						Diagnostic::new("only a struct can be boxed into a reference", inner.1.into_range())
							.with_label(format!("this is {typ}")),
					);
				};
				// move the literal's slots into a shared box
				let n = fields.len();
				let base = self.call_alloc_bytes((n * 8) as i64 + 16);
				let descv = self.trace_desc(name, fields);
				self.b.ins().store(MemFlags::new(), descv, base, 0);
				let one = self.b.ins().iconst(self.int, 1);
				self.b.ins().store(MemFlags::new(), one, base, 8);
				let boxp = self.b.ins().iadd_imm(base, 16);
				for i in 0..n {
					let v = self.b.ins().load(self.int, MemFlags::new(), ptr, (i * 8) as i32);
					self.b.ins().store(MemFlags::new(), v, boxp, (i * 8) as i32);
				}
				let typ = Typ::Ref(Box::new(typ.clone()));
				self.temp(boxp, &typ);
				Ok((boxp, typ))
			}

			Expr::Record(entries) => self.record_lit(entries, expr.1, None),

			Expr::Map(entries) => self.map_lit(entries, expr.1, None),

			Expr::Range { start, end } => {
				let start_val = match start {
					Some(s) => self.int_value(s, "range start")?,
					None => self.b.ins().iconst(cl_int_for_width(32), 0),
				};
				let end_val = match end {
					Some(e) => self.int_value(e, "range end")?,
					None => self.b.ins().iconst(cl_int_for_width(32), 0),
				};
				let ptr = self.call_alloc(2);
				let cl = self.b.func.dfg.value_type(start_val);
				let s_ext = if cl == self.int {
					start_val
				} else {
					self.b.ins().sextend(self.int, start_val)
				};
				let e_ext = if cl == self.int {
					end_val
				} else {
					self.b.ins().sextend(self.int, end_val)
				};
				self.b.ins().store(MemFlags::new(), s_ext, ptr, 0);
				self.b.ins().store(MemFlags::new(), e_ext, ptr, 8);
				Ok((ptr, Typ::Range))
			}

			Expr::AnonFn {
				captures,
				params,
				params_tuple,
				ret,
				body,
			} => {
				let Some(ret) = ret else {
					return Err(Diagnostic::new(
						"anonymous functions need an explicit return type",
						expr.1.into_range(),
					)
					.with_label("add a return type, e.g. `fn [] () int { ... }`"));
				};
				self.declare_anon_fn(captures, params, *params_tuple, AnonSig::Explicit(ret), body, expr.1)
			}

			Expr::Bind { .. } => unreachable!("bind in expression position"),
			Expr::Assign { .. } => unreachable!("assign in expression position"),
			Expr::Destructure { .. } => unreachable!("destructuring in expression position"),
			Expr::IndexAssign { .. } => unreachable!("index assign in expression position"),
			Expr::Fn { .. } => unreachable!("fn definition in expression position"),
			Expr::StructDef { .. } => unreachable!("struct definition in expression position"),
			Expr::EnumDef { .. } => unreachable!("enum definition in expression position"),
			Expr::Claim { .. } => unreachable!("claim in expression position"),
			Expr::TraitDef { .. } => unreachable!("trait definition in expression position"),
			Expr::TypeAlias { .. } => unreachable!("type alias in expression position"),
			Expr::FieldAssign { .. } => unreachable!("field assign in expression position"),
			Expr::Return(..) => unreachable!("return in expression position"),
			Expr::Break | Expr::Continue => unreachable!("break/continue in expression position"),
			Expr::Append { .. } => unreachable!("append in expression position"),
			Expr::MapDelete { .. } => unreachable!("map delete in expression position"),
			Expr::Doc(_) | Expr::Module(_) | Expr::Use { .. } | Expr::Pub(_) => unreachable!("not an expression"),
		}
	}
}
