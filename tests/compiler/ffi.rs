use indoc::indoc;

use crate::helpers::check;

#[test]
fn scalars_cross_ptr() {
	let src = indoc! {"
		buf: []u8 = .[0, 0, 0, 0, 0, 0, 0, 0]
		unsafe buf.ptr.write(42)
		print(unsafe buf.ptr.read[i32]())
		unsafe buf.ptr.write(buf.ptr)
		print(unsafe buf.ptr.read[ptr]().is_null())
	"};
	check(src, ["42", "false"]);
}

#[test]
fn fn_casts_to_ptr() {
	let src = indoc! {"
		@c
		cb :: fn(n: i32) i32 { n + 1 }
		Cb :: @c fn(n: i32) i32
		f := unsafe Cb(ptr(cb))
		print(f(41), ptr(0).is_null())
	"};
	check(src, "42 true");
}
