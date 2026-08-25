use crate::helpers::*;

#[test]
fn annotation_on_fn() {
	check(
		indoc! {r#"
			deprecated :: struct { reason: string }
			@:awesome
			@deprecated.{reason = "use speak()"}
			pub yell :: fn() string { "AAA" }
			print(yell())
		"#},
		"AAA",
	);
}

#[test]
fn field_annotation_trailing_form() {
	check(
		indoc! {r#"
			Player :: struct { name: string @:required }
			p := Player.{ name = "Todd" }
			print(p.name)
		"#},
		"Todd",
	);
}

#[test]
fn bare_const_marker_from_core() {
	check(
		indoc! {r#"
			User :: struct { name: string @required }
			u := User.{ name = "Todd" }
			print(u.name)
		"#},
		"Todd",
	);
}

#[test]
fn main_file_const_as_annotation() {
	check(
		indoc! {r#"
			opt :: ()
			deprecated :: struct { reason: string }
			warn :: deprecated.{reason = "old"}
			@warn
			speak :: fn() string { "hi" }
			User :: struct { name: string @opt }
			u := User.{ name = "Todd" }
			print("{speak()} {u.name}")
		"#},
		"hi Todd",
	);
}

#[test]
fn builtin_marker_is_a_real_value() {
	check("print(required)", "()");
}

#[test]
fn unknown_annotation_is_not_a_struct() {
	fail_with(
		indoc! {"
			@bogus.{}
			thing :: fn() int { 1 }
		"},
		"`bogus` is not a struct",
	);
}

#[test]
fn annotation_arg_must_be_literal() {
	fail_with(
		indoc! {r#"
			deprecated :: struct { reason: string }
			x :: "hi"
			@deprecated.{x}
			thing :: fn() int { 1 }
		"#},
		"annotation arguments must be literal values",
	);
}

#[test]
fn annotation_field_set_twice() {
	fail_with(
		indoc! {r#"
			deprecated :: struct { reason: string }
			@deprecated.{"a", reason = "b"}
			thing :: fn() int { 1 }
		"#},
		"`reason` was already set positionally",
	);
}

#[test]
fn annotation_only_goes_on_definitions() {
	fail_with(
		indoc! {"
			Foo :: struct {}
			@:awesome
			Foo : Eq {}
		"},
		"annotations only attach to definitions",
	);
}

#[test]
fn bare_struct_name_is_not_a_value() {
	fail_with(
		indoc! {"
			deprecated :: struct { reason: string }
			@deprecated
			thing :: fn() int { 1 }
		"},
		"is a struct, not a value",
	);
}
