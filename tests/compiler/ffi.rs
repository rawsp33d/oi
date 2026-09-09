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
