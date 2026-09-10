use crate::helpers::fail_with;

#[test]
fn vararg_rejections() {
	for src in [
		"f :: fn(x: ?...int) {}",
		"f :: fn(x: []...int) {}",
		"S :: struct { xs: ...int }",
		"f :: fn() ...int { [] }",
	] {
		fail_with(src, "`...T` is only allowed as a parameter type");
	}
	fail_with("f :: fn(a: ...int, b: ...int) {}", "`f` has more than one vararg");
}
