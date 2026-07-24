use crate::helpers::*;

#[test]
fn tail_return() {
	check(
		indoc! {"
			type Status = :ok | :err
			fn f() Status { :err }
			f()
		"},
		"err",
	);
}

#[test]
fn bind_and_print() {
	check(
		indoc! {"
			type Status = :ok | :err
			x Status := :ok
			x
		"},
		"ok",
	);
}

#[test]
fn zero_value_is_first_variant() {
	check(
		indoc! {"
			type Status = :ok | :err
			mut x Status
			x
		"},
		"ok",
	);
}

#[test]
fn eq() {
	check(
		indoc! {"
			type Status = :ok | :err
			a Status := :ok
			b Status := :ok
			a == b
		"},
		"true",
	);
	check(
		indoc! {"
			type Status = :ok | :err
			a Status := :ok
			b Status := :err
			a == b
		"},
		"false",
	);
	check(
		indoc! {"
			type Status = :ok | :err
			a Status := :ok
			b Status := :err
			a != b
		"},
		"true",
	);
}

#[test]
fn matching() {
	check(
		indoc! {r#"
			type Status = :ok | :err
			x Status := :err
			match x {
				:ok => "good",
				:err => "bad",
			}
		"#},
		"bad",
	);
	check(
		indoc! {r#"
			type Status = :ok | :err
			x Status := :err
			match x {
				:ok => "good",
				_ => "fallback",
			}
		"#},
		"fallback",
	);
	let err = fail(indoc! {r#"
			type Status = :ok | :err
			x Status := :err
			match x {
				:ok => "good",
			}
		"#});
	assert!(err.contains("non-exhaustive match, missing: err"), "got: {err}");
}

#[test]
fn unknown_atom_errors() {
	let err = fail(indoc! {"
		type Status = :ok | :err
		x Status := :nope
	"});
	assert!(err.contains("has no atom `:nope`"), "got: {err}");
}

#[test]
fn duplicate_atom_in_type_errors() {
	let err = fail(indoc! {"
		type Status = :ok | :ok
		fn f() Status { :ok }
		f()
	"});
	assert!(err.contains("duplicate atom `:ok` in sum type"), "got: {err}");
}

#[test]
fn int_cast_gives_tag() {
	check(
		indoc! {"
			type Status = :ok | :err
			x Status := :ok
			int(x)
		"},
		"0",
	);
	check(
		indoc! {"
			type Status = :ok | :err
			x Status := :err
			int(x)
		"},
		"1",
	);
}

#[test]
fn struct_field_type() {
	check(
		indoc! {"
			type Status = :ok | :err
			struct Res { s Status }
			r := Res{ s: :err }
			r.s
		"},
		"err",
	);
}

#[test]
fn anonymous_sum_in_signature() {
	check(
		indoc! {"
			fn f() :ok | :err { :err }
			f()
		"},
		"err",
	);
}

#[test]
fn anonymous_sum_in_bind() {
	check(
		indoc! {"
			x :ok | :err := :ok
			x
		"},
		"ok",
	);
}

#[test]
fn anonymous_sum_param_and_return() {
	check(
		indoc! {"
			fn f(v int | string) int | string { v }
			match f(7) {
				n @ int => n + 1,
				string => 0,
			}
		"},
		"8",
	);
	check(
		indoc! {r#"
			fn f(v int | string) int | string { v }
			match f("hi") {
				int => "no",
				s @ string => s,
			}
		"#},
		"hi",
	);
}

#[test]
fn tight_prefix_precedence() {
	check(
		indoc! {"
			type V = :none | []int | :other
			mut x V := :other
			int(x)
		"},
		"2",
	);
	check(
		indoc! {"
			type V = :none | []int | :other
			mut x V := :none
			x = [1, 2]
			int(x)
		"},
		"1",
	);
}

#[test]
fn general_bind_print_and_zero() {
	check(
		indoc! {"
			type Id = int | string
			x Id := 7
			x
		"},
		"7",
	);
	// zero value is the first member's zero
	check(
		indoc! {"
			type Id = int | string
			mut x Id
			x
		"},
		"0",
	);
}

#[test]
fn general_reassign_across_members() {
	check(
		indoc! {r#"
			type Id = int | string
			mut x Id := 7
			x = "hi"
			x
		"#},
		"hi",
	);
}

#[test]
fn general_fn_return_and_field() {
	check(
		indoc! {"
			type Id = int | string
			fn make() Id { 42 }
			make()
		"},
		"42",
	);
	check(
		indoc! {r#"
			type Id = int | string
			struct Box { id Id }
			Box{ id: "hey" }.id
		"#},
		"hey",
	);
}

#[test]
fn mixed_atom_and_type() {
	check(
		indoc! {"
			type V = :none | int
			mut x V := :none
			x
		"},
		"none",
	);
	check(
		indoc! {"
			type V = :none | int
			mut x V := :none
			x = 5
			x
		"},
		"5",
	);
}

#[test]
fn general_match() {
	check(
		indoc! {"
			type Id = int | string
			x Id := 7
			match x {
				n @ int => n + 1,
				string => 0,
			}
		"},
		"8",
	);
	check(
		indoc! {r#"
			type Id = int | string
			x Id := "z"
			match x {
				int => 1,
				else => 2,
			}
		"#},
		"2",
	);
	let err = fail(indoc! {"
		type Id = int | string
		x Id := 7
		match x {
			string => 0,
		}
	"});
	assert!(err.contains("non-exhaustive match, missing: int"), "got: {err}");
}

#[test]
fn general_eq_is_structural() {
	check(
		indoc! {"
			type Id = int | string
			a Id := 7
			b Id := 7
			a == b
		"},
		"true",
	);
	check(
		indoc! {r#"
			type Id = int | string
			a Id := 7
			b Id := "x"
			a == b
		"#},
		"false",
	);
	check(
		indoc! {"
			type A = int | string
			type B = int | string
			a A := 1
			b B := 1
			a == b
		"},
		"true",
	);
}

#[test]
fn set_identity() {
	check(
		indoc! {"
			type A = int | string
			type B = string | int
			a A := 7
			b B := a
			match b {
				n @ int => n + 1,
				string => 0,
			}
		"},
		"8",
	);
	check(
		indoc! {"
			type A = int | string
			type B = string | int
			a A := 7
			b B := a
			a == b
		"},
		"true",
	);
	check(
		indoc! {"
			type A = int | string
			a A := 7
			int(a)
		"},
		"0",
	);
	check(
		indoc! {"
			type A = int | string
			type B = string | int
			a A := 7
			b B := a
			int(b)
		"},
		"1",
	);
	check(
		indoc! {r#"
			type B = string | int
			mut x B
			x == ""
		"#},
		"true",
	);
}

#[test]
fn general_int_cast_gives_tag() {
	check(
		indoc! {r#"
			type Id = int | string
			x Id := "x"
			int(x)
		"#},
		"1",
	);
}

#[test]
fn duplicate_type_member_errors() {
	let err = fail(indoc! {"
		type Bad = int | int
		mut x Bad
		x
	"});
	assert!(err.contains("duplicate member `int` in sum type"), "got: {err}");
}

#[test]
fn nested_sum_alias_splices() {
	check(
		indoc! {r#"
			type Num = int | f64
			type Value = Num | string
			x Value := 7
			match x {
				int => 1,
				f64 => 2,
				string => 3,
			}
		"#},
		"1",
	);
	check(
		indoc! {r#"
			type Num = int | f64
			type Value = Num | string
			x Value := "hi"
			match x {
				int => 1,
				f64 => 2,
				string => 3,
			}
		"#},
		"3",
	);
}

#[test]
fn splice_duplicate_member_errors() {
	let err = fail(indoc! {"
		type Num = int | f64
		type Bad = Num | int
		mut x Bad
		x
	"});
	assert!(err.contains("duplicate member `int` in sum type"), "got: {err}");
}

#[test]
fn single_type_stays_transparent_alias() {
	check(
		indoc! {"
			type Score = int
			x Score := 5
			x + 1
		"},
		"6",
	);
}
