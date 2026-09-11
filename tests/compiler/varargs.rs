use crate::helpers::*;

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
	fail_with(
		"f :: fn(a: ...int, b: int) {}\nf()",
		"`f` expects 1.. argument(s), got 0",
	);
	fail_with("f :: fn(xs: ...int) {}\na :: [1]\nf(a)", "expected int, got []int");
}

#[test]
fn varargs() {
	let src = indoc! {r#"
		sum :: fn(xs: ...int) int {
			t := 0
			loop x in xs { t = t + x }
			t
		}
		between :: fn(open: string, items: ...string, close: string) string { "{open}{items.len}{close}" }
		join :: fn(parts: ...string, sep := ",") string { "{parts.len}{sep}" }
		count :: fn(args: ...any) int { args.len }
		s :: sum
		ps :: ["a", "b"]
		print(sum(1, 2, 3), sum(), s(4, 5), count(1, "a", true))
		print(between("[", "a", "b", "]"), between("[", "]"), join("a", "b", sep = "/"))
		print(join(...ps), join("pre", ...ps, "post"), sum(...[1, 2]))
	"#};
	check(src, ["6 0 9 3", "[2] [0] 2/", "2, 4, 3"]);
}

#[test]
fn generic_varargs() {
	let src = indoc! {r#"
		count[T] :: fn(xs: ...T) int { xs.len }
		first[T] :: fn(xs: ...T) T { xs[0] }
		print(count(1, 2, 3), count("a", "b"))
		print(first(1, 2, 3), first("a", "b"))
	"#};
	check(src, ["3 2", "1 a"]);
	fail_with(
		"first[T] :: fn(xs: ...T) T { xs[0] }\nfirst(1, \"a\")",
		"array elements must share a type",
	);
}
