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

#[test]
fn required_field_omitted() {
	fail_with(
		indoc! {"
			Foo :: struct { n: int @required }
			f := Foo.{}
		"},
		"`Foo.n` is required",
	);
}

#[test]
fn required_field_provided() {
	check(
		indoc! {r#"
			Foo :: struct { n: int @required }
			a := Foo.{n = 1}
			b := Foo.{2}
			c := Foo.{...a}
			print("{a.n} {b.n} {c.n}")
		"#},
		"1 2 1",
	);
}

#[test]
fn generic_required_omitted() {
	fail_with(
		indoc! {"
			Box[T] :: struct { v: T, n: int @required }
			b := Box.{v = 1}
		"},
		"is required",
	);
}

#[test]
fn params_omits_trailing_struct() {
	check(
		indoc! {"
			@params
			Settings :: struct { idk: int = 7 }
			User :: struct {}
			User :{
				with_settings :: fn(self, settings: Settings) { print(settings.idk) }
			}
			take :: fn(settings: Settings) { print(settings.idk) }
			u := User.{}
			u.with_settings()
			take()
			take(.{idk = 1})
		"},
		["7", "7", "1"],
	);
}

#[test]
fn params_only_on_marked_struct() {
	fail_with(
		indoc! {"
			Settings :: struct { idk: int = 7 }
			take :: fn(settings: Settings) { print(settings.idk) }
			take()
		"},
		"expects 1 argument",
	);
}

#[test]
fn params_synthesized_literal_checks_required() {
	fail_with(
		indoc! {"
			@params
			Settings :: struct { idk: int @required }
			take :: fn(settings: Settings) { print(settings.idk) }
			take()
		"},
		"is required",
	);
}
