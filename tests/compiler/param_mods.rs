use crate::helpers::*;

#[test]
fn array_inout() {
	check(
		indoc! {"
			fn push9(mut xs []int) { xs << 9 }
			mut a := [1]
			push9(mut a)
			a
		"},
		"[1, 9]",
	);
}

#[test]
fn array_reassign_inout() {
	check(
		indoc! {"
			fn swap(mut xs []int) { xs = [7, 8] }
			mut a := [1]
			swap(mut a)
			a
		"},
		"[7, 8]",
	);
}

#[test]
fn map_inout() {
	check(
		indoc! {r#"
			fn setk(mut m Map[string, int]) { m["k"] = 1 }
			mut m := {a: 0}
			setk(mut m)
			m["k"]
		"#},
		"1",
	);
}

#[test]
fn struct_inout() {
	check(
		indoc! {"
			struct C { n int }
			fn bump(mut c C) { c.n = c.n + 1 }
			mut c := C{n: 1}
			bump(mut c)
			c.n
		"},
		"2",
	);
}

#[test]
fn generic_inout() {
	check(
		indoc! {"
			fn push[T](mut xs []T, v T) []T {
				xs << v
				xs
			}
			mut a := [1]
			push(mut a, 9)
			a
		"},
		"[1, 9]",
	);
}

#[test]
fn method_mut_arg() {
	check(
		indoc! {"
			struct C { n int }
			impl C { fn take(self, mut xs []int) { xs << self.n } }
			mut a := [1]
			c := C{n: 7}
			c.take(mut a)
			a
		"},
		"[1, 7]",
	);
}

#[test]
fn mut_self_on_mut_binding() {
	check(
		indoc! {"
			struct C { n int }
			impl C { fn bump(mut self) { self.n = self.n + 1 } }
			mut c := C{n: 1}
			c.bump()
			c.n
		"},
		"2",
	);
}

#[test]
fn slice_projection_element_write() {
	check(
		indoc! {"
			fn set(mut a []int) { a[0] = 9 }
			mut xs := [1, 2, 3, 4]
			set(mut xs[1..3])
			xs
		"},
		"[1, 9, 3, 4]",
	);
}

#[test]
fn slice_projection_is_leak_free() {
	// the callee frees the copy, the caller frees its replacement
	assert_clean(indoc! {"
		fn swap(mut a []int) { a = [7, 8] }
		mut xs := [1, 2, 3, 4]
		swap(mut xs[1..3])
		print(xs)
	"});
}

#[test]
fn inout_is_leak_free() {
	assert_clean(indoc! {"
		fn push9(mut xs []int) { xs << 9 }
		fn swap(mut xs []int) { xs = [7, 8] }
		mut a := [1]
		push9(mut a)
		swap(mut a)
		print(a)
	"});
}

// errors

#[test]
fn missing_mut_at_callsite() {
	fail_with(
		["fn f(mut xs []int) {}", "mut a := [1]", "f(a)"],
		"missing `mut` at the callsite",
	);
}

#[test]
fn mut_on_non_mut_param() {
	fail_with(["fn f(xs []int) {}", "mut a := [1]", "f(mut a)"], "not `mut`");
}

#[test]
fn immutable_binding_lent() {
	fail_with(
		indoc! {"
			fn f(mut xs []int) {}
			a := [1]
			f(mut a)
		"},
		"declared without `mut`",
	);
}

#[test]
fn non_place_lent() {
	fail_with(["fn f(mut xs []int) {}", "f(mut [1, 2])"], "only a mutable binding");
}

#[test]
fn exclusivity_same_name() {
	fail_with(
		indoc! {"
			fn f(mut xs []int, ys []int) {}
			mut a := [1]
			f(mut a, a)
		"},
		"while it is lent `mut`",
	);
}

#[test]
fn exclusivity_in_subexpression() {
	fail_with(
		["fn f(mut xs []int, n int) {}", "mut a := [1]", "f(mut a, a[0])"],
		"while it is lent `mut`",
	);
}

#[test]
fn exclusivity_covers_receiver() {
	fail_with(
		indoc! {"
			struct C { xs []int }
			impl C { fn take(self, mut xs []int) {} }
			mut c := C{xs: [1]}
			c.take(mut c)
		"},
		"while it is lent `mut`",
	);
}

#[test]
fn mut_self_needs_mut_binding() {
	fail_with(
		indoc! {"
			struct C { n int }
			impl C { fn bump(mut self) { self.n = self.n + 1 } }
			c := C{n: 1}
			c.bump()
		"},
		"needs a `mut` binding",
	);
}

#[test]
fn slice_projection_length_change_panics() {
	fail_with(
		indoc! {"
			fn grow(mut a []int) { a = [7, 8, 9] }
			mut xs := [1, 2, 3, 4]
			grow(mut xs[1..3])
		"},
		"projection changed length",
	);
}

#[test]
fn slice_projection_exclusivity() {
	fail_with(
		indoc! {"
			fn f(mut a []int, b int) {}
			mut xs := [1, 2, 3]
			f(mut xs[1..3], xs[0])
		"},
		"while it is lent `mut`",
	);
}

#[test]
fn slice_projection_immutable_base_rejected() {
	fail_with(
		indoc! {"
			fn f(mut a []int) {}
			xs := [1, 2, 3]
			f(mut xs[1..3])
		"},
		"declared without `mut`",
	);
}

#[test]
fn scalar_mut_param_rejected() {
	fail_with("fn f(mut n int) {}", "must be arrays, maps, or structs");
}

#[test]
fn callee_cannot_mutate_plain_param() {
	fail_with("fn f(xs []int) { xs << 1 }", "immutable");
}
