//! Functions a compiled Oi program calls at runtime. Backend-agnostic: the JIT
//! registers them as symbols, an object backend would link them in.

pub const PRINT_INT: &str = "oi_print_int";
pub const PRINT_FLOAT: &str = "oi_print_float";

pub extern "C" fn print_int(x: i64) {
	println!("{x}");
}

pub extern "C" fn print_float(x: f64) {
	println!("{x:?}");
}
