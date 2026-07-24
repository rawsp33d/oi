use crate::helpers::*;

#[test]
fn paren_call_trailing_fn() {
	let src = indoc! {"
		fn retry(n int, f fn() int) int { f() }
		retry(2) fn() int { 21 }
	"};
	check(src, "21");
}

#[test]
fn bare_block_desugars_to_anon_fn() {
	let src = indoc! {"
		fn retry(n int, f fn() int) int { f() }
		retry(2) { 21 }
	"};
	assert!(fail(src).contains("explicit return type"));
}

#[test]
fn trailing_only_no_parens() {
	let src = indoc! {"
		fn twice(f fn() int) int { f() + f() }
		twice fn() int { 21 }
	"};
	check(src, "42");
}

#[test]
fn method_trailing_fn() {
	let src = indoc! {"
		struct Box { n int }
		impl Box {
			fn with(self, f fn() int) int { self.n + f() }
			fn m(self, k int, f fn() int) int { self.n + k + f() }
		}
		b := Box{ n: 10 }
		print(b.with fn() int { 5 })
		b.m(1) fn() int { 5 }
	"};
	check(src, "15\n16");
}

#[test]
fn headers_stay_juxt_free() {
	let src = indoc! {"
		cond := true
		if cond { print(1) }
		mut i := 0
		loop i < 3 { i = i + 1 }
		print(i)
		x := 5
		match x { 5 => print(9), else => print(0) }
	"};
	check(src, "1\n3\n9");
}

#[test]
fn bind_rhs_trailing_fn() {
	let src = indoc! {"
		fn twice(f fn() int) int { f() + f() }
		x := twice fn() int { 21 }
		x
	"};
	check(src, "42");
}
