use crate::helpers::*;

#[test]
fn qualified_access() {
	check("Color :: enum { red green blue }\nColor.red", "red");
	check("Color :: enum { red green blue }\nColor.blue", "blue");
}

#[test]
fn oneliner() {
	check("Fruit :: enum { apple orange grape }\nFruit.orange", "orange");
}

#[test]
fn bind() {
	check("Color :: enum { red green blue }\nc :: Color.green\nc", "green");
}

#[test]
fn reassign() {
	check(
		indoc! {"
			Color :: enum { red green blue }
			c := Color.red
			c = Color.blue
			c
		"},
		"blue",
	);
}

#[test]
fn first_variant_is_default() {
	check("Color :: enum { red green blue }\nc: Color\nc", "red");
}

#[test]
fn empty_literal_is_default() {
	check("Color :: enum { red green blue }\nColor{}", "red");
}

#[test]
fn empty_literal_rejects_fields() {
	fail_with("Color :: enum { red green blue }\nColor{ red }", "only supports");
}

#[test]
fn eq_same() {
	check("Color :: enum { red green blue }\nColor.red == Color.red", "true");
}

#[test]
fn eq_different() {
	check("Color :: enum { red green blue }\nColor.red == Color.blue", "false");
}

#[test]
fn ne() {
	check("Color :: enum { red green blue }\nColor.red != Color.blue", "true");
}

#[test]
fn returned_from_fn() {
	check(
		"Color :: enum { red green blue }\nfav :: fn() Color { Color.blue }\nfav()",
		"blue",
	);
}

#[test]
fn struct_field() {
	check(
		"Stat :: enum { health mana stamina }
		User :: struct { s: Stat }
		u :: User{ s: Stat.mana }
		u.s",
		"mana",
	);
}

#[test]
fn in_match() {
	check(
		indoc! {r#"
			Color :: enum { red green blue }
			c :: Color.green
			match c {
				Color.red => "r",
				Color.green => "g",
				else => "?",
			}
		"#},
		"g",
	);
}

#[test]
fn unknown_variant() {
	fail_with("Color :: enum { red green blue }\nColor.purple", "no variant `purple`");
}

#[test]
fn shorthand_in_assignment() {
	check(
		indoc! {"
			Color :: enum { red green blue }
			c := Color.green
			c = .red
			c
		"},
		"red",
	);
}

#[test]
fn shorthand_in_annotated_binding() {
	check("Color :: enum { red green blue }\nc : Color : .blue\nc", "blue");
}

#[test]
fn shorthand_in_comparison() {
	check("Color :: enum { red green blue }\nc :: Color.red\nc == .red", "true");
	check("Color :: enum { red green blue }\nc :: Color.red\nc != .blue", "true");
}

#[test]
fn shorthand_in_match() {
	check(
		indoc! {r#"
			Color :: enum { red green blue }
			c :: Color.green
			match c {
				.red => "r",
				.green => "g",
				else => "?",
			}
		"#},
		"g",
	);
}

#[test]
fn shorthand_in_struct_field() {
	check(
		"Stat :: enum { health mana stamina }
		User :: struct { s: Stat }
		u :: User{ s: .mana }
		u.s",
		"mana",
	);
	check(
		"Stat :: enum { health mana stamina }
		User :: struct { s: Stat }
		u :: User{ .stamina }
		u.s",
		"stamina",
	);
}

#[test]
fn shorthand_unknown_variant() {
	fail_with(
		"Color :: enum { red green blue }\nc :: Color.red\nc == .purple",
		"no variant `purple`",
	);
}

#[test]
fn shorthand_without_context_errors() {
	fail_with("Color :: enum { red green blue }\n.red", "cannot infer the enum type");
}

#[test]
fn duplicate_disc_rejected() {
	fail_with("E :: enum { a = 2, b, c = 2 }", "discriminant value `2`");
}

#[test]
fn auto_increment_from_explicit() {
	fail_with("E :: enum { a = 5, b, c = 6 }", "discriminant value `6`");
}

#[test]
fn negative_disc() {
	fail_with("E :: enum { a = -2, b, c = -1 }", "discriminant value `-1`");
}

#[test]
fn payload_construct() {
	check(
		"Shape :: enum { point triangle(f64, f64, f64) }\nShape.triangle(3.0, 4.0, 5.0)",
		"triangle(3.0, 4.0, 5.0)",
	);
}

#[test]
fn payloadless_variant_of_boxed_enum() {
	check("Opt :: enum { nope some(int) }\nOpt.nope", "nope");
	check("Opt :: enum { nope some(int) }\no : Opt : .nope\no", "nope");
}

#[test]
fn payload_enum_default_is_first() {
	check("Opt :: enum { nope some(int) }\no: Opt\no", "nope");
}

#[test]
fn payload_empty_literal_is_default() {
	check("Opt :: enum { nope some(int) }\nOpt{}", "nope");
}

#[test]
fn payload_int_cast_errors() {
	fail_with("Opt :: enum { nope some(int) }\nint(Opt.some(1))", "no backing value");
}

#[test]
fn payload_field_type_mismatch() {
	fail_with("Opt :: enum { nope some(int) }\nOpt.some(3.0)", "expected int, got float");
}

#[test]
fn payload_wrong_arity() {
	fail_with("Opt :: enum { nope some(int) }\nOpt.some()", "takes 1 field(s), got 0");
}

#[test]
fn payload_match_binds_fields() {
	check(
		indoc! {r#"
			Opt :: enum { nope some(int) }
			o :: Opt.some(7)
			match o {
				.some(n) => n,
				.nope => -1,
			}
		"#},
		"7",
	);
}

#[test]
fn payload_match_fieldless_arm() {
	check(
		indoc! {r#"
			Opt :: enum { nope some(int) }
			o : Opt : .nope
			match o {
				.some(n) => n,
				.nope => -1,
			}
		"#},
		"-1",
	);
}

#[test]
fn payload_match_multiple_fields() {
	check(
		indoc! {r#"
			Shape :: enum { rect(int, int) tri(int, int, int) }
			s :: Shape.rect(3, 4)
			match s {
				.rect(w, h) => w * h,
				.tri(a, b, c) => a + b + c,
			}
		"#},
		"12",
	);
}

#[test]
fn shorthand_payload_construct() {
	check(
		"Opt :: enum { nope some(int) }\no : Opt : .some(5)\nmatch o { .some(n) => n, .nope => 0 }",
		"5",
	);
}

#[test]
fn payload_eq() {
	check("Opt :: enum { nope some(int) }\nOpt.some(1) == Opt.some(1)", "true");
	check("Opt :: enum { nope some(int) }\nOpt.some(1) == Opt.some(2)", "false");
	check("Opt :: enum { nope some(int) }\nOpt.nope == Opt.some(1)", "false");
	check("Opt :: enum { nope some(int) }\nOpt.nope != Opt.some(1)", "true");
}

#[test]
fn payload_eq_string_field() {
	check(
		indoc! {r#"
			Msg :: enum { quit say(str) }
			Msg.say("hi") == Msg.say("hi")
		"#},
		"true",
	);
	check(
		indoc! {r#"
			Msg :: enum { quit say(str) }
			Msg.say("hi") == Msg.say("bye")
		"#},
		"false",
	);
}

#[test]
fn payload_ordering_rejected() {
	fail_with(
		"Opt :: enum { nope some(int) }\nOpt.some(1) < Opt.some(2)",
		"only `==`&`!=`",
	);
}

#[test]
fn struct_payload() {
	check(
		indoc! {r#"
			Point :: struct { x: int, y: int }
			Shape :: enum { dot rect(Point) }
			s :: Shape.rect(Point{ x: 3, y: 4 })
			match s {
				.rect(p) => print(p),
				.dot => {}
			}
		"#},
		"Point{x: 3, y: 4}",
	);
}

#[test]
fn enum_payload() {
	check(
		indoc! {r#"
			A :: enum { one two }
			B :: enum { wrap(A) empty }
			b :: B.wrap(A.two)
			match b {
				.wrap(a) => match a {
					.one => "one",
					.two => "two",
				},
				.empty => "none",
			}
		"#},
		"two",
	);
}

#[test]
fn struct_form_construct_and_match() {
	check(
		indoc! {r#"
			Shape :: enum {
				circle { radius: f64 }
				rectangle { width: f64, height: f64 }
				triangle(f64, f64, f64)
				point
			}
			s :: Shape.circle { radius: 5.0 }
			match s {
				.circle { radius } => radius * 2.0,
				.rectangle { width, height } => width * height,
				.triangle(a, b, c) => a + b + c,
				.point => 0.0,
			}
		"#},
		"10.0",
	);
}

#[test]
fn struct_form_shorthand_and_rename() {
	check(
		indoc! {r#"
			Shape :: enum { circle { radius: f64 } rectangle { width: f64, height: f64 } }
			mk :: fn() Shape { .rectangle { width: 3.0, height: 4.0 } }
			match mk() {
				.rectangle { width: w, height } => w * height,
				else => 0.0,
			}
		"#},
		"12.0",
	);
}

#[test]
fn struct_form_zero_is_first_variant() {
	check(
		indoc! {r#"
			Shape :: enum { circle { radius: f64 } rectangle { width: f64, height: f64 } }
			match Shape{} { .circle { radius } => radius, else => -1.0 }
		"#},
		"0.0",
	);
}

#[test]
fn struct_form_unknown_field() {
	fail_with("S :: enum { circle { radius: f64 } }\nS.circle { r: 1.0 }", "no field `r`");
}

#[test]
fn struct_form_omitted_field_zeroes() {
	check(
		indoc! {r#"
			S :: enum { rect { w: f64, h: f64 } }
			s :: S.rect { h: 2.0 }
			match s { .rect { w, h } => w + h }
		"#},
		"2.0",
	);
}

#[test]
fn struct_form_positional_rejected() {
	fail_with("S :: enum { circle { radius: f64 } }\nS.circle(1.0)", "takes named fields");
}

#[test]
fn tuple_form_record_rejected() {
	fail_with("S :: enum { tri(f64, f64) }\nS.tri { a: 1.0 }", "takes 2 field(s), got 1");
}

#[test]
fn alias_payload() {
	check(
		indoc! {"
			Meters :: f64
			Dist :: enum { unknown known(Meters) }
			d :: Dist.known(5.0)
			match d {
				.known(m) => m,
				.unknown => 0.0,
			}
		"},
		"5.0",
	);
}

#[test]
fn payload_unknown_type_rejected() {
	fail_with("A :: enum { wrap(NoSuchType) }", "unknown type");
}

#[test]
fn explicit_disc_default_is_first() {
	check("E :: enum { a = 5, b c }\nx: E\nx", "a");
}

#[test]
fn atom_coerces_in_annotated_binding() {
	check("Color :: enum { red green blue }\nc : Color : :blue\nc", "blue");
}

#[test]
fn atom_coerces_in_assignment() {
	check(
		indoc! {"
			Color :: enum { red green blue }
			c := Color.green
			c = :red
			c
		"},
		"red",
	);
}

#[test]
fn atom_coerces_in_comparison() {
	check("Color :: enum { red green blue }\nc :: Color.red\nc == :red", "true");
	check("Color :: enum { red green blue }\nColor.blue == :blue", "true");
}

#[test]
fn atom_coerces_in_struct_field() {
	check(
		indoc! {"
			Stat :: enum { health mana stamina }
			User :: struct { s: Stat }
			u :: User{ s: :mana }
			u.s
		"},
		"mana",
	);
}

#[test]
fn atom_unknown_variant() {
	fail_with(
		"Color :: enum { red green blue }\nc : Color : :purple",
		"no variant `purple`",
	);
}

#[test]
fn cast_to_int() {
	check("Color :: enum { red green blue }\nint(Color.blue)", "2");
}

#[test]
fn cast_to_int_explicit_disc() {
	check("Status :: enum { ok = 200, err = 500 }\nint(Status.err)", "500");
}

#[test]
fn backed_cast_to_backing() {
	check("Status : u8 : enum { ok = 200, err = 250 }\nu8(Status.ok)", "200");
}

#[test]
fn backing_non_integer_errors() {
	fail_with("E : bool : enum { a }", "not an enum-able type");
}

#[test]
fn backing_out_of_range_errors() {
	fail_with("E : u8 : enum { a = 300 }", "out of range for its backing type");
}

#[test]
fn backing_with_payload_errors() {
	fail_with("E : u8 : enum { a some(int) }", "cannot have payload");
}

#[test]
fn compare_via_int() {
	check("Color :: enum { red green blue }\nint(Color.green) == 1", "true");
}

#[test]
fn ord_gives_discriminant() {
	check("Color :: enum { red green blue }\nord(Color.blue)", "2");
}

#[test]
fn ord_on_payload_variant() {
	check("Opt :: enum { nope some(int) }\nord(Opt.some(1))", "1");
}

#[test]
fn str_method() {
	check("Color :: enum { red green blue }\nColor.blue.str()", "blue");
}

#[test]
fn str_method_concat() {
	check(
		r#"Color :: enum { red green blue }
		"the color is " + Color.green.str()"#,
		"the color is green",
	);
}

#[test]
fn no_such_method() {
	fail_with("Color :: enum { red green blue }\nColor.red.hex()", "has no method `hex`");
}

#[test]
fn from_int_match() {
	check("Color :: enum { red green blue }\nColor.from(1) or { Color.red }", "green");
}

#[test]
fn from_int_no_match() {
	check("Color :: enum { red green blue }\nColor.from(9) or { Color.red }", "red");
}

#[test]
fn from_int_no_match_carries_error() {
	check(
		"Color :: enum { red green blue }\nColor.from(9) or { print($)\nColor.red }",
		["no matching variant", "red"],
	);
}

#[test]
fn from_str_match() {
	check(
		"Color :: enum { red green blue }\nColor.from(\"blue\") or { Color.red }",
		"blue",
	);
}

#[test]
fn from_str_no_match() {
	check(
		"Color :: enum { red green blue }\nColor.from(\"purple\") or { print($)\nColor.red }",
		"no matching variant\nred",
	);
}

#[test]
fn from_atom_match() {
	check(
		"Color :: enum { red green blue }\nColor.from(:blue) or { Color.red }",
		"blue",
	);
}

#[test]
fn from_atom_no_match() {
	check(
		"Color :: enum { red green blue }\nColor.from(:purple) or { print($)\nColor.red }",
		["no matching variant", "red"],
	);
}

#[test]
fn from_payload_zero_fills() {
	check(
		"Shape :: enum { point triangle(f64, f64, f64) }\nShape.from(1) or { Shape.point }",
		"triangle(0.0, 0.0, 0.0)",
	);
}

#[test]
fn from_wrong_type() {
	fail_with(
		"Color :: enum { red green blue }\nColor.from(true)",
		"needs an int, str, or atom",
	);
}

#[test]
fn shorthand_coerces_in_fn_arg() {
	check(
		indoc! {"
			Color :: enum { red green blue }
			name :: fn(c: Color) string { c.str() }
			name(.blue)
		"},
		"blue",
	);
}

#[test]
fn atom_coerces_in_fn_arg() {
	check(
		indoc! {"
			Color :: enum { red green blue }
			name :: fn(c: Color) string { c.str() }
			name(:blue)
		"},
		"blue",
	);
}

#[test]
fn shorthand_coerces_in_if_tail_return() {
	check(
		indoc! {"
			Color :: enum { red green blue }
			fav :: fn(pick: bool) Color {
				if pick { .blue } else { .red }
			}
			fav(true)
		"},
		"blue",
	);
}

#[test]
fn shorthand_coerces_in_match_tail_return() {
	check(
		indoc! {r#"
			Color :: enum { red green blue }
			fav :: fn(n: int) Color {
				match n {
					1 => .red,
					else => .blue,
				}
			}
			fav(9)
		"#},
		"blue",
	);
}

#[test]
fn shorthand_coerces_in_if_expr() {
	check(
		indoc! {"
			Color :: enum { red green blue }
			c : Color : if false { .red } else { .blue }
			c
		"},
		"blue",
	);
}

#[test]
fn shorthand_coerces_in_match_expr() {
	check(
		indoc! {r#"
			Color :: enum { red green blue }
			n :: 9
			c : Color : match n {
				1 => .red,
				else => .blue,
			}
			c
		"#},
		"blue",
	);
}

#[test]
fn print_payloads() {
	check(
		indoc! {"
			Shape :: enum {
				point
				circle { radius: f64 }
				triangle(f64, f64, f64)
			}
			print(Shape.point)
			print(Shape.circle { radius: 5.0 })
			print(Shape.triangle(3.0, 4.0, 5.0))
		"},
		["point", "circle{radius: 5.0}", "triangle(3.0, 4.0, 5.0)"],
	);
}

#[test]
fn backed_arrays_pack() {
	check(
		indoc! {"
			Status : u8 : enum { ok = 200, err = 250 }
			a := [Status.ok, Status.err]
			a[0] = Status.err
			a << Status.ok
			print(a)
			loop s in a { print(s) }
			print(Status.err in a)
			print(match a { [x, y, z] => z, else => Status.err })
			f: [3]Status
			f[1] = Status.err
			print(f[1])
			f[0] == Status.ok
		"},
		["[err, err, ok]", "err", "err", "ok", "true", "ok", "err", "true"],
	);
}

#[test]
fn string_backed_raws() {
	check(
		indoc! {r#"
			Suit : string : enum { hearts = "♥" spades = "♠" }
			print(string(Suit.spades))
			print(str(Suit.hearts))
			print(Suit.spades.str())
			print(ord(Suit.spades))
			print(Suit.hearts == Suit.hearts)
			a :: [Suit.spades, Suit.hearts]
			print(str(a[1]))
			match Suit.spades { .spades => "s", else => "?" }
		"#},
		["♠", "♥", "spades", "1", "true", "♥", "s"],
	);
	check("S : string : enum { a b }\nstr(S.b)", "b");
}

#[test]
fn string_backed_errors() {
	fail_with("S : string : enum { a b }\nint(S.a)", "cannot cast str");
	fail_with("S :: enum { a = \"x\" }", "needs a string backing");
	fail_with("S : string : enum { a = 2 }", "uses raw values");
	fail_with("S : string : enum { a = \"x\" b = \"x\" }", "assigned more than once");
	fail_with("S : string : enum { a b = \"a\" }", "assigned more than once");
}

#[test]
fn backed_array_signed_sextends() {
	check(
		indoc! {r#"
			Delta : i8 : enum { down = -3, up = 4 }
			a :: [Delta.up, Delta.down]
			d :: a[1]
			print(int(d))
			print(match d { Delta.down => "yes", else => "no" })
			d == Delta.down
		"#},
		["-3", "yes", "true"],
	);
}
