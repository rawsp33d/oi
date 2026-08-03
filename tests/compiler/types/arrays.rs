use crate::helpers::*;
use indoc::indoc;

#[test]
fn array_literal() {
	check("[1, 2, 3]", "[1, 2, 3]");
}

#[test]
fn fn_param_type() {
	let src = indoc! {"
		fn first(xs []int) int {
			xs.0
		}
		first([9, 8, 7])
	"};
	check(src, "9");
}

#[test]
fn array_of_strings() {
	// strings are quoted when printed inside an array
	check(r#"["a", "b"]"#, r#"["a", "b"]"#);
}

#[test]
fn array_of_floats() {
	check("[1.5, 2.5]", "[1.5, 2.5]");
}

#[test]
fn array_of_bools() {
	check("[true, false, true]", "[true, false, true]");
}

#[test]
fn single_element() {
	check("[42]", "[42]");
}

#[test]
fn trailing_comma() {
	check("[1, 2,]", "[1, 2]");
}

#[test]
fn no_comma_ints() {
	check("[2 4 6]", "[2, 4, 6]");
}

#[test]
fn no_comma_strings() {
	check(r#"["a" "b" "c"]"#, r#"["a", "b", "c"]"#);
}

#[test]
fn no_comma_bools() {
	check("[true false true]", "[true, false, true]");
}

#[test]
fn no_comma_mixed_with_nested() {
	// spec example: `odd << [9 11]`
	check("mut odd := [1, 3, 5]\nodd << [9 11]\nodd", "[1, 3, 5, 9, 11]");
}

#[test]
fn no_comma_in_slice_literal() {
	// spec shows `[2 4]` as a comma-free array literal
	check("a := [2 4]\nassert(a[0] == 2)\na[1]", "4");
}

#[test]
fn no_comma_loop() {
	// spec example: `loop x in [2 4 6 8]`
	check(
		indoc! {"
		mut sum := 0
		loop x in [2 4 6 8] { sum = sum + x }
		sum
	"},
		"20",
	);
}

#[test]
fn index_literal() {
	check("a := [10, 20, 30]\na[1]", "20");
}

#[test]
fn index_variable() {
	check("a := [10, 20, 30]\ni := 2\na[i]", "30");
}

#[test]
fn index_expression() {
	// the index is any Int expression
	check("a := [10, 20, 30]\na[1 + 1]", "30");
}

#[test]
fn dot_index() {
	// numeric dot notation indexes like `[n]`
	check("a := [10, 20, 30]\na.0", "10");
}

#[test]
fn len_field() {
	check("a := [10, 20, 30]\na.len", "3");
}

#[test]
fn index_arithmetic() {
	check("a := [3, 4]\na[0] * a[1]", "12");
}

#[test]
fn array_in_var_prints() {
	check(r#"a := [1, 2, 3]; a"#, "[1, 2, 3]");
}

#[test]
fn nested_in_tuple() {
	// a tuple prints an array element without a trailing newline
	check(r#"(1, [2, 3], "x")"#, r#"(1, [2, 3], "x")"#);
}

#[test]
fn array_of_tuples() {
	// elements print through the same path as everything else, so composites recurse
	check("[(1, 2), (3, 4)]", "[(1, 2), (3, 4)]");
}

#[test]
fn nested_arrays() {
	check("a := [10, 20]\nb := [30, 40]\n[a, b]", "[[10, 20], [30, 40]]");
}

#[test]
fn index_into_nested() {
	check("a := [10, 20]\nb := [30, 40]\n[a, b][1]", "[30, 40]");
}

#[test]
fn mixed_types() {
	fail_with(r#"[1, "two"]"#, "must share a type");
}

#[test]
fn empty_unsupported() {
	fail_with("[]", "empty array");
}

#[test]
fn index_non_array() {
	fail_with("x := 5\nx[0]", "cannot index");
}

#[test]
fn non_int_index() {
	fail_with(r#"a := [1, 2]; a["x"]"#, "index must be Int");
}

#[test]
fn index_out_of_range() {
	// out-of-range indexing aborts at runtime
	fail_with("a := [1, 2]\na[5]", "out of range");
}

#[test]
fn unknown_named_field() {
	fail_with("a := [1, 2]\na.foo", "no field `foo`");
}

// slices

#[test]
fn slice_middle() {
	// a half-open range: indices 1 and 2
	check("even := [0, 2, 4, 6, 8]\neven[1..3]", "[2, 4]");
}

#[test]
fn slice_from_start() {
	// an omitted start defaults to 0
	check("even := [0, 2, 4, 6, 8]\neven[..3]", "[0, 2, 4]");
}

#[test]
fn slice_to_end() {
	// an omitted end defaults to the length
	check("even := [0, 2, 4, 6, 8]\neven[1..]", "[2, 4, 6, 8]");
}

#[test]
fn slice_full() {
	check("even := [0, 2, 4, 6, 8]\neven[..]", "[0, 2, 4, 6, 8]");
}

#[test]
fn slice_empty() {
	// an empty range yields an empty array (the only way to make one for now)
	check("even := [0, 2, 4, 6, 8]\neven[2..2]", "[]");
}

#[test]
fn slice_variable_bounds() {
	check(["a := [0, 2, 4, 6, 8]", "lo := 1", "hi := 4", "a[lo..hi]"], "[2, 4, 6]");
}

#[test]
fn slice_is_an_array() {
	// the result is a normal array: indexable, with a `len`, and re-sliceable
	check(
		indoc! {"
		a := [0, 2, 4, 6, 8]
		assert(a[1..][0] == 2)
		assert(a[1..3].len == 2)
	"},
		"true",
	);
}

#[test]
fn slice_of_strings() {
	check(r#"a := ["w", "x", "y", "z"]; a[1..3]"#, r#"["x", "y"]"#);
}

#[test]
fn slice_of_tuples() {
	check("a := [(1, 2), (3, 4), (5, 6)]\na[..2]", "[(1, 2), (3, 4)]");
}

#[test]
fn slice_out_of_bounds() {
	fail_with("a := [1, 2, 3]\na[1..9]", "out of bounds");
}

#[test]
fn slice_reversed_range() {
	fail_with("a := [1, 2, 3]\na[3..1]", "out of bounds");
}

#[test]
fn slice_non_array() {
	fail_with("x := 5\nx[0..1]", "cannot slice");
}

#[test]
fn slice_non_int_bound() {
	fail_with(r#"a := [1, 2, 3]; a[true..2]"#, "must be Int");
}

// index assignment

#[test]
fn index_assign_basic() {
	check("mut a := [1, 2, 3]\na[1] = 99\na", "[1, 99, 3]");
}

#[test]
fn index_assign_first() {
	check("mut a := [10, 20]\na[0] = 1\na[0]", "1");
}

#[test]
fn index_assign_variable_index() {
	check(["mut a := [1, 2, 3]", "mut i := 2", "a[i] = 7", "a"], "[1, 2, 7]");
}

#[test]
fn index_assign_multiple() {
	check(
		["mut a := [0, 0, 0]", "a[0] = 1", "a[1] = 2", "a[2] = 3", "a"],
		"[1, 2, 3]",
	);
}

#[test]
fn index_assign_strings() {
	check(r#"mut a := ["x", "y"]; a[0] = "hello"; a"#, r#"["hello", "y"]"#);
}

#[test]
fn index_assign_slice_is_independent() {
	check(
		indoc! {"
		mut a := [1, 2, 3]
		b := a[1..]
		a[1] = 99
		assert(b[0] == 2)
	"},
		"true",
	);
}

#[test]
fn index_assign_immutable_error() {
	fail_with("a := [1, 2]\na[0] = 5", "immutable");
}

#[test]
fn index_assign_non_array_error() {
	fail_with("mut x := 5\nx[0] = 1", "not an array");
}

#[test]
fn index_assign_type_mismatch_error() {
	fail_with(r#"mut a := [1, 2]; a[0] = "hi""#, "type mismatch");
}

#[test]
fn index_assign_oob_error() {
	fail_with("mut a := [1, 2]\na[5] = 9", "out of range");
}

// append

#[test]
fn append_basic() {
	check("mut a := [1, 2, 3]\na << 4\na", "[1, 2, 3, 4]");
}

#[test]
fn append_updates_len() {
	check(["mut a := [1, 3, 5]", "a << 7", "a << 9", "a.len"], "5");
}

#[test]
fn append_multiple() {
	check(["mut a := [0]", "a << 1", "a << 2", "a << 3", "a"], "[0, 1, 2, 3]");
}

#[test]
fn append_strings() {
	check(r#"mut a := ["x"]; a << "y"; a << "z"; a"#, r#"["x", "y", "z"]"#);
}

#[test]
fn append_grows_past_initial_cap() {
	// initial cap == len == 2; force multiple doublings
	check(
		["mut a := [1, 2]", "a << 3", "a << 4", "a << 5", "a"],
		"[1, 2, 3, 4, 5]",
	);
}

#[test]
fn append_slice_copies_buffer() {
	// appending to a slice forces a copy; the parent is unaffected
	check(["a := [1, 2, 3]", "mut b := a[1..]", "b << 99", "b"], "[2, 3, 99]");
}

#[test]
fn append_immutable_error() {
	fail_with("a := [1, 2]\na << 3", "immutable");
}

#[test]
fn append_non_array_error() {
	fail_with("mut x := 5\nx << 1", "not an array");
}

#[test]
fn append_type_mismatch_error() {
	fail_with(r#"mut a := [1, 2]; a << "hi""#, "type mismatch");
}

// array extend (<<)

#[test]
fn extend_basic() {
	check("mut odd := [1, 3, 5]\nodd << [9, 11]\nodd", "[1, 3, 5, 9, 11]");
}

#[test]
fn extend_updates_len() {
	check("mut odd := [1, 3, 5]\nodd << [9, 11]\nodd.len", "5");
}

#[test]
fn extend_empty_src() {
	// appending a zero-length slice leaves dst unchanged
	check(["mut a := [1, 2, 3]", "b := a[0..0]", "a << b", "a"], "[1, 2, 3]");
}

#[test]
fn extend_into_empty_ish() {
	// extend a slice (cap == len) by another array
	check(["a := [1, 2]", "mut b := a[0..0]", "b << [3, 4]", "b"], "[3, 4]");
}

#[test]
fn extend_type_mismatch_error() {
	fail_with(r#"mut a := [1, 2]; b := ["x"]; a << b"#, "type mismatch");
}

// value semantics (COW)

#[test]
fn copy_is_independent_on_append() {
	check(
		indoc! {"
		mut a := [1, 2, 3]
		b := a
		a << 4
		print(a)
		b
	"},
		["[1, 2, 3, 4]", "[1, 2, 3]"],
	);
}

#[test]
fn appended_element_is_independent_copy() {
	check(
		indoc! {"
		mut inner := [1]
		mut outer := [[9]]
		outer << inner
		inner << 2
		outer[1]
	"},
		"[1]",
	);
}

#[test]
fn index_assign_copy_forward() {
	check(["mut a := [1, 2, 3]", "b := a", "a[0] = 99", "b"], "[1, 2, 3]");
}

#[test]
fn index_assign_copy_backward() {
	check(["a := [1, 2, 3]", "mut b := a", "b[0] = 99", "a"], "[1, 2, 3]");
}

#[test]
fn chain_of_copies_is_independent() {
	check(
		indoc! {"
		mut a := [1, 2, 3]
		b := a
		c := b
		a << 4
		print(b)
		c
	"},
		["[1, 2, 3]", "[1, 2, 3]"],
	);
}

#[test]
fn copy_of_zero_value_appends() {
	check(["a := []int{}", "mut b := a", "b << 1", "b"], "[1]");
}

#[test]
fn slice_independent_from_parent() {
	check(["mut a := [1, 2, 3]", "b := a[..]", "a[0] = 99", "b"], "[1, 2, 3]");
}

#[test]
fn parent_independent_from_slice() {
	check(["a := [1, 2, 3]", "mut b := a[..]", "b[0] = 99", "a"], "[1, 2, 3]");
}

#[test]
fn returned_param_independent_from_arg() {
	check(
		indoc! {"
		fn id(a []int) []int { a }
		a := [1, 2, 3]
		mut r := id(a)
		r << 4
		a
	"},
		"[1, 2, 3]",
	);
}

// in operator

#[test]
fn in_found() {
	check("even := [0, 2, 4, 6, 8]\n6 in even", "true");
}

#[test]
fn in_not_found() {
	check("even := [0, 2, 4, 6, 8]\n5 in even", "false");
}

#[test]
fn in_first_element() {
	check("a := [10, 20, 30]\n10 in a", "true");
}

#[test]
fn in_last_element() {
	check("a := [10, 20, 30]\n30 in a", "true");
}

#[test]
fn in_strings() {
	check(r#"a := ["w", "x", "y"]; "x" in a"#, "true");
}

#[test]
fn in_strings_not_found() {
	check(r#"a := ["w", "x", "y"]; "z" in a"#, "false");
}

#[test]
fn in_after_append() {
	check(["mut a := [1, 2]", "a << 3", "3 in a"], "true");
}

#[test]
fn in_non_array_error() {
	fail_with("5 in 10", "not an array");
}

#[test]
fn in_type_mismatch_error() {
	fail_with(r#"a := [1, 2]; "x" in a"#, "type mismatch");
}

#[test]
fn fn_returns_array_annotation() {
	let src = indoc! {"
		fn nums() []int { [1, 2, 3] }
		nums()
	"};
	check(src, "[1, 2, 3]");
}

#[test]
fn fn_returns_array_field() {
	let src = indoc! {"
		fn nums() []int { [10, 20, 30] }
		a := nums()
		a[1]
	"};
	check(src, "20");
}

#[test]
fn fn_return_type_mismatch_array() {
	let src = indoc! {"
		fn bad() []int { 42 }
		bad()
	"};
	fail_with(src, "wrong return type");
}

#[test]
fn if_no_else_array_zero() {
	let src = indoc! {"
		a := if false { [1, 2, 3] }
		a.len
	"};
	check(src, "0");
}

// fixed-size arrays

#[test]
fn fixed_zeroed() {
	check("[3]int{}", "[0, 0, 0]");
}

#[test]
fn fixed_len_is_constant() {
	check("a := [4]int{}\na.len", "4");
}

#[test]
fn fixed_index_write_read() {
	check(
		r#"mut three := [3]string{}
		three[0] = "larry"
		three[1] = "curly"
		three"#,
		r#"["larry", "curly", ""]"#,
	);
}

#[test]
fn fixed_dot_index() {
	check(["mut a := [2]int{}", "a[0] = 9", "a.0"], "9");
}

#[test]
fn fixed_annotated_zero_decl() {
	check(
		r#"mut grid [3]string
grid"#,
		r#"["", "", ""]"#,
	);
}

#[test]
fn fixed_value_semantics() {
	check(["mut a := [2]int{}", "mut b := a", "a[0] = 9", "b[0]"], "0");
}

#[test]
fn fixed_index_out_of_range() {
	fail_with("a := [2]int{}\na[5]", "out of range");
}

#[test]
fn empty_dynamic_via_init() {
	check("a := []int{}\na.len", "0");
}

#[test]
fn typed_literal() {
	check("[]int{1, 2}", "[1, 2]");
}

#[test]
fn typed_literal_index() {
	check("a := []int{10, 20, 30}\na[1]", "20");
}

#[test]
fn typed_literal_mismatch() {
	fail_with(r#"a := []int{1, "x"}"#, "share a type");
}
