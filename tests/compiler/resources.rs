use crate::helpers::*;

const FILE: &str = indoc! {r#"
	File :: struct { fd: int }
	File : Drop < { drop :: fn(mut self) { print("drop", self.fd) } }
"#};

#[test]
fn reverse_drop_order() {
	check([FILE, "a :: File.{fd = 1}", "b :: File.{fd = 2}"], ["drop 2", "drop 1"]);
}

#[test]
fn bind_move_kills_source() {
	fail_with([FILE, "f :: File.{fd = 1}", "g :: f", "print(f)"], "undefined variable");
}

#[test]
fn arg_borrows_and_drops_once() {
	check(
		[
			FILE,
			"look :: fn(f: File) {}",
			"f :: File.{fd = 1}",
			"look(f)",
			r#"print("marker")"#,
		],
		["marker", "drop 1"],
	);
}

#[test]
fn callee_cannot_steal_a_borrowed_arg() {
	fail_with(
		[
			FILE,
			"steal :: fn(f: File) { g :: f }",
			"f :: File.{fd = 1}",
			"steal(f)",
		],
		"it is borrowed here",
	);
}

#[test]
fn returned_resource_drops_once() {
	check(
		[
			FILE,
			"open :: fn(n: int) File { File.{fd = n} }",
			"f :: open(3)",
			r#"print("before")"#,
		],
		["before", "drop 3"],
	);
}

#[test]
fn resource_field_makes_its_owner_one() {
	let owner = "Handle :: struct { file: File }";
	check(
		[FILE, owner, "h :: Handle.{file = File.{fd = 1}}", r#"print("built")"#],
		["built", "drop 1"],
	);
	fail_with(
		[FILE, owner, "h :: Handle.{file = File.{fd = 1}}", "g :: h", "print(h)"],
		"undefined variable",
	);
	fail_with(
		[FILE, owner, "f :: File.{fd = 1}", "h :: Handle.{file = f}", "print(f)"],
		"undefined variable",
	);
}

#[test]
fn array_elements_drop_with_their_last_owner() {
	check(
		[
			FILE,
			"a :: [File.{fd = 1}, File.{fd = 2}]",
			"b :: a",
			r#"print("built")"#,
		],
		["built", "drop 1", "drop 2"],
	);
	fail_with(
		[FILE, "f :: File.{fd = 1}", "a :: [f]", "print(f)"],
		"undefined variable",
	);
}

#[test]
fn a_projected_resource_is_a_borrow() {
	let a = "a :: [File.{fd = 1}]";
	check(
		[FILE, a, "look :: fn(f: File) {}", "look(a[0])", "print(a[0].fd)"],
		["1", "drop 1"],
	);
	fail_with([FILE, a, "g :: a[0]"], "cannot move File out of its container");
}

#[test]
fn map_values_drop_with_their_last_owner() {
	let f = "f :: File.{fd = 1}";
	check([FILE, f, r#"m :: ["a" = f]"#, r#"print("built")"#], ["built", "drop 1"]);
	check(
		[FILE, "m: Map[string, File]", f, r#"m["a"] = f"#, r#"print("set")"#],
		["set", "drop 1"],
	);
}

#[test]
fn fixed_array_elements_drop_with_their_last_owner() {
	let a = "a : [2]File : .[File.{fd = 1}, File.{fd = 2}]";
	check([FILE, a, "b :: a", r#"print("built")"#], ["built", "drop 1", "drop 2"]);
	fail_with([FILE, a, "b :: a", "print(a[0].fd)"], "undefined variable");
}
