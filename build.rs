// Export `oi_*` functions for `dlsym`.
fn main() {
	println!("cargo:rustc-link-arg-bins=-rdynamic");
}
