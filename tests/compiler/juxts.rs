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
	fail_with(src, "explicit return type");
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
		b :: Box{ n: 10 }
		print(b.with fn() int { 5 })
		b.m(1) fn() int { 5 }
	"};
	check(src, "15\n16");
}

#[test]
fn leading_literals() {
	let src = indoc! {"
		struct Box { n int }
		impl Box { fn tag(self, a :go) int { self.n } }
		fn shout(s string) string { s }
		fn take(n int) int { n }
		print(shout \"hey\")
		print(take 1_000)
		Box{ n: 10 }.tag :go
	"};
	check(src, "hey\n1000\n10");
}

#[test]
fn literal_and_trailing_fn() {
	let src = indoc! {"
		fn run_test(name string, f fn() int) int { print(name) f() }
		run_test \"reg\" fn() int { 21 }
	"};
	check(src, "reg\n21");
}

#[test]
fn headers_stay_juxt_free() {
	let src = indoc! {"
		cond :: true
		if cond { print(1) }
		i := 0
		loop i < 3 { i = i + 1 }
		print(i)
		x :: 5
		match x { 5 => print(9), else => print(0) }
	"};
	check(src, "1\n3\n9");
}

#[test]
fn call_then_literal_return() {
	let src = indoc! {"
		fn logret() string { print(1) \"done\" }
		logret()
	"};
	check(src, "1\ndone");
}

#[test]
fn bind_rhs_trailing_fn() {
	let src = indoc! {"
		fn twice(f fn() int) int { f() + f() }
		x :: twice fn() int { 21 }
		x
	"};
	check(src, "42");
}

#[test]
fn array_elem_juxt() {
	let src = indoc! {"
		fn double(n int) int { n * 2 }
		a :: [double 3]
		a.len
	"};
	check(src, "1");
}
