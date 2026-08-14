use crate::helpers::*;

const FILE: &str = indoc! {r#"
	File :: struct { fd: int }
	File : Drop { drop :: fn(mut self) { print("drop", self.fd) } }
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
