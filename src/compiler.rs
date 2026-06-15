use std::collections::HashMap;

use cranelift::codegen;
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, Linkage, Module};

use crate::ast::Expr;
use crate::runtime;

pub struct Compiler {
	builder_ctx: FunctionBuilderContext,
	ctx: codegen::Context,
	data_description: DataDescription,
	module: JITModule,
}

impl Default for Compiler {
	fn default() -> Self {
		let mut flag_builder = settings::builder();
		flag_builder.set("use_colocated_libcalls", "false").unwrap();
		flag_builder.set("is_pic", "false").unwrap();
		let isa = cranelift_native::builder()
			.unwrap_or_else(|e| panic!("unsupported host: {e}"))
			.finish(settings::Flags::new(flag_builder))
			.unwrap();
		let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
		builder.symbol(runtime::PRINT_INT, runtime::print_int as *const u8);
		builder.symbol(runtime::PRINT_FLOAT, runtime::print_float as *const u8);

		let module = JITModule::new(builder);
		Self {
			builder_ctx: FunctionBuilderContext::new(),
			ctx: module.make_context(),
			data_description: DataDescription::new(),
			module,
		}
	}
}

impl Compiler {
	pub fn compile(&mut self, program: &[Expr]) -> Result<*const u8, String> {
		let int = self.module.target_config().pointer_type();
		self.translate(int, program)?;

		let id = self
			.module
			.declare_function("__oi_main", Linkage::Local, &self.ctx.func.signature)
			.map_err(|e| e.to_string())?;

		self.module
			.define_function(id, &mut self.ctx)
			.map_err(|e| e.to_string())?;

		self.module.clear_context(&mut self.ctx);
		self.module.finalize_definitions().unwrap();

		let code = self.module.get_finalized_function(id);
		Ok(code)
	}

	fn translate(&mut self, int: types::Type, program: &[Expr]) -> Result<(), String> {
		let mut b = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_ctx);
		let block = b.create_block();
		b.switch_to_block(block);
		b.seal_block(block);

		let mut trans = Translator {
			int,
			b,
			vars: HashMap::new(),
			module: &mut self.module,
			string_idx: 0,
		};

		let mut last = trans.b.ins().iconst(int, 0);
		for stmt in program {
			match stmt {
				Expr::Assign { name, value, .. } => {
					let val = trans.expr(value);
					// a variable takes the type of its first assigned value
					let var = match trans.vars.get(name) {
						Some(&var) => var,
						None => {
							let typ = trans.b.func.dfg.value_type(val);
							let var = trans.b.declare_var(typ);
							trans.vars.insert(name.clone(), var);
							var
						}
					};
					trans.b.def_var(var, val);
				}
				e => last = trans.expr(e),
			}
		}

		trans.emit_print(last);
		trans.b.ins().return_(&[]);
		trans.b.finalize();
		Ok(())
	}
}

enum Op {
	Add,
	Sub,
	Mul,
	Div,
}

struct Translator<'a> {
	int: types::Type,
	b: FunctionBuilder<'a>,
	vars: HashMap<String, Variable>,
	module: &'a mut JITModule,
	string_idx: usize,
}

impl<'a> Translator<'a> {
	fn expr(&mut self, expr: &Expr) -> Value {
		match expr {
			Expr::Int(n) => self.b.ins().iconst(self.int, *n as i64),
			Expr::Bool(v) => self.b.ins().iconst(self.int, *v as i64),
			Expr::Float(x) => self.b.ins().f64const(*x),

			Expr::String(s) => {
				let mut bytes = s.as_bytes().to_vec();
				bytes.push(0);
				let name = format!("__str_{}", self.string_idx);
				self.string_idx += 1;
				let id = self
					.module
					.declare_data(&name, Linkage::Local, false, false)
					.unwrap();
				let mut desc = DataDescription::new();
				desc.define(bytes.into_boxed_slice());
				self.module.define_data(id, &desc).unwrap();
				let gv = self.module.declare_data_in_func(id, &mut self.b.func);
				self.b.ins().symbol_value(self.int, gv)
			}

			Expr::Ident(name) => self.b.use_var(
				*self
					.vars
					.get(name)
					.unwrap_or_else(|| panic!("undefined: {name}")),
			),

			Expr::Negative(e) => {
				let v = self.expr(e);
				match self.b.func.dfg.value_type(v) {
					types::F64 => self.b.ins().fneg(v),
					_ => self.b.ins().ineg(v),
				}
			}

			Expr::Add(l, r) => self.binop(Op::Add, l, r),
			Expr::Sub(l, r) => self.binop(Op::Sub, l, r),
			Expr::Mul(l, r) => self.binop(Op::Mul, l, r),
			Expr::Div(l, r) => self.binop(Op::Div, l, r),

			Expr::Assign { .. } => unreachable!("assign in expression position"),
		}
	}

	// Add binary op instruction.
	fn binop(&mut self, op: Op, l: &Expr, r: &Expr) -> Value {
		let lv = self.expr(l);
		let rv = self.expr(r);
		let float = self.b.func.dfg.value_type(lv) == types::F64;
		// make sure both sides match
		// NOTE: For now. I might go with V-style promotion eventually.
		if float != (self.b.func.dfg.value_type(rv) == types::F64) {
			panic!("cannot mix int and float; cast explicitly");
		}
		let b = self.b.ins();
		match (op, float) {
			(Op::Add, true) => b.fadd(lv, rv),
			(Op::Add, false) => b.iadd(lv, rv),
			(Op::Sub, true) => b.fsub(lv, rv),
			(Op::Sub, false) => b.isub(lv, rv),
			(Op::Mul, true) => b.fmul(lv, rv),
			(Op::Mul, false) => b.imul(lv, rv),
			(Op::Div, true) => b.fdiv(lv, rv),
			(Op::Div, false) => b.sdiv(lv, rv),
		}
	}

	// Emit a call to the runtime print function for the result's type.
	fn emit_print(&mut self, val: Value) {
		let (name, param) = if self.b.func.dfg.value_type(val) == types::F64 {
			(runtime::PRINT_FLOAT, types::F64)
		} else {
			(runtime::PRINT_INT, self.int)
		};
		let mut sig = self.module.make_signature();
		sig.params.push(AbiParam::new(param));
		let id = self
			.module
			.declare_function(name, Linkage::Import, &sig)
			.unwrap();
		let func = self.module.declare_func_in_func(id, self.b.func);
		self.b.ins().call(func, &[val]);
	}
}
