use crate::helpers::*;

#[test]
fn qualified_access() {
	check("enum Color { red green blue }\nColor.red", "red");
	check("enum Color { red green blue }\nColor.blue", "blue");
}

#[test]
fn oneliner() {
	check("enum Fruit { apple orange grape }\nFruit.orange", "orange");
}

#[test]
fn bind() {
	check("enum Color { red green blue }\nc :: Color.green\nc", "green");
}

#[test]
fn reassign() {
	check(
		indoc! {"
			enum Color { red green blue }
			c := Color.red
			c = Color.blue
			c
		"},
		"blue",
	);
}

#[test]
fn first_variant_is_default() {
	check("enum Color { red green blue }\nc: Color\nc", "red");
}

#[test]
fn empty_literal_is_default() {
	check("enum Color { red green blue }\nColor{}", "red");
}

#[test]
fn empty_literal_rejects_fields() {
	fail_with("enum Color { red green blue }\nColor{ red }", "only supports");
}

#[test]
fn eq_same() {
	check("enum Color { red green blue }\nColor.red == Color.red", "true");
}

#[test]
fn eq_different() {
	check("enum Color { red green blue }\nColor.red == Color.blue", "false");
}

#[test]
fn ne() {
	check("enum Color { red green blue }\nColor.red != Color.blue", "true");
}

#[test]
fn returned_from_fn() {
	check(
		"enum Color { red green blue }\nfn fav() Color { Color.blue }\nfav()",
		"blue",
	);
}

#[test]
fn struct_field() {
	check(
		"enum Stat { health mana stamina }
		struct User { s Stat }
		u :: User{ s: Stat.mana }
		u.s",
		"mana",
	);
}

#[test]
fn in_match() {
	check(
		indoc! {r#"
			enum Color { red green blue }
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
	fail_with("enum Color { red green blue }\nColor.purple", "no variant `purple`");
}

#[test]
fn shorthand_in_assignment() {
	check(
		indoc! {"
			enum Color { red green blue }
			c := Color.green
			c = .red
			c
		"},
		"red",
	);
}

#[test]
fn shorthand_in_annotated_binding() {
	check("enum Color { red green blue }\nc : Color : .blue\nc", "blue");
}

#[test]
fn shorthand_in_comparison() {
	check("enum Color { red green blue }\nc :: Color.red\nc == .red", "true");
	check("enum Color { red green blue }\nc :: Color.red\nc != .blue", "true");
}

#[test]
fn shorthand_in_match() {
	check(
		indoc! {r#"
			enum Color { red green blue }
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
		"enum Stat { health mana stamina }
		struct User { s Stat }
		u :: User{ s: .mana }
		u.s",
		"mana",
	);
	check(
		"enum Stat { health mana stamina }
		struct User { s Stat }
		u :: User{ .stamina }
		u.s",
		"stamina",
	);
}

#[test]
fn shorthand_unknown_variant() {
	fail_with(
		"enum Color { red green blue }\nc :: Color.red\nc == .purple",
		"no variant `purple`",
	);
}

#[test]
fn shorthand_without_context_errors() {
	fail_with("enum Color { red green blue }\n.red", "cannot infer the enum type");
}

#[test]
fn duplicate_disc_rejected() {
	fail_with("enum E { a = 2, b, c = 2 }", "discriminant value `2`");
}

#[test]
fn auto_increment_from_explicit() {
	fail_with("enum E { a = 5, b, c = 6 }", "discriminant value `6`");
}

#[test]
fn negative_disc() {
	fail_with("enum E { a = -2, b, c = -1 }", "discriminant value `-1`");
}

#[test]
fn payload_construct() {
	check(
		"enum Shape { point triangle(f64, f64, f64) }\nShape.triangle(3.0, 4.0, 5.0)",
		"triangle(3.0, 4.0, 5.0)",
	);
}

#[test]
fn payloadless_variant_of_boxed_enum() {
	check("enum Opt { nope some(int) }\nOpt.nope", "nope");
	check("enum Opt { nope some(int) }\no : Opt : .nope\no", "nope");
}

#[test]
fn payload_enum_default_is_first() {
	check("enum Opt { nope some(int) }\no: Opt\no", "nope");
}

#[test]
fn payload_empty_literal_is_default() {
	check("enum Opt { nope some(int) }\nOpt{}", "nope");
}

#[test]
fn payload_int_cast_errors() {
	fail_with("enum Opt { nope some(int) }\nint(Opt.some(1))", "no backing value");
}

#[test]
fn payload_field_type_mismatch() {
	fail_with("enum Opt { nope some(int) }\nOpt.some(3.0)", "expected int, got float");
}

#[test]
fn payload_wrong_arity() {
	fail_with("enum Opt { nope some(int) }\nOpt.some()", "takes 1 field(s), got 0");
}

#[test]
fn payload_match_binds_fields() {
	check(
		indoc! {r#"
			enum Opt { nope some(int) }
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
			enum Opt { nope some(int) }
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
			enum Shape { rect(int, int) tri(int, int, int) }
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
		"enum Opt { nope some(int) }\no : Opt : .some(5)\nmatch o { .some(n) => n, .nope => 0 }",
		"5",
	);
}

#[test]
fn payload_eq() {
	check("enum Opt { nope some(int) }\nOpt.some(1) == Opt.some(1)", "true");
	check("enum Opt { nope some(int) }\nOpt.some(1) == Opt.some(2)", "false");
	check("enum Opt { nope some(int) }\nOpt.nope == Opt.some(1)", "false");
	check("enum Opt { nope some(int) }\nOpt.nope != Opt.some(1)", "true");
}

#[test]
fn payload_eq_string_field() {
	check(
		indoc! {r#"
			enum Msg { quit say(str) }
			Msg.say("hi") == Msg.say("hi")
		"#},
		"true",
	);
	check(
		indoc! {r#"
			enum Msg { quit say(str) }
			Msg.say("hi") == Msg.say("bye")
		"#},
		"false",
	);
}

#[test]
fn payload_ordering_rejected() {
	fail_with(
		"enum Opt { nope some(int) }\nOpt.some(1) < Opt.some(2)",
		"only `==`&`!=`",
	);
}

#[test]
fn struct_payload() {
	check(
		indoc! {r#"
			struct Point { x int, y int }
			enum Shape { dot rect(Point) }
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
			enum A { one two }
			enum B { wrap(A) empty }
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
			enum Shape {
				circle { radius f64 }
				rectangle { width f64, height f64 }
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
			enum Shape { circle { radius f64 } rectangle { width f64, height f64 } }
			fn mk() Shape { .rectangle { width: 3.0, height: 4.0 } }
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
			enum Shape { circle { radius f64 } rectangle { width f64, height f64 } }
			match Shape{} { .circle { radius } => radius, else => -1.0 }
		"#},
		"0.0",
	);
}

#[test]
fn struct_form_unknown_field() {
	fail_with("enum S { circle { radius f64 } }\nS.circle { r: 1.0 }", "no field `r`");
}

#[test]
fn struct_form_omitted_field_zeroes() {
	check(
		indoc! {r#"
			enum S { rect { w f64, h f64 } }
			s :: S.rect { h: 2.0 }
			match s { .rect { w, h } => w + h }
		"#},
		"2.0",
	);
}

#[test]
fn struct_form_positional_rejected() {
	fail_with("enum S { circle { radius f64 } }\nS.circle(1.0)", "takes named fields");
}

#[test]
fn tuple_form_record_rejected() {
	fail_with("enum S { tri(f64, f64) }\nS.tri { a: 1.0 }", "takes 2 field(s), got 1");
}

#[test]
fn alias_payload() {
	check(
		indoc! {"
			type Meters = f64
			enum Dist { unknown known(Meters) }
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
	fail_with("enum A { wrap(NoSuchType) }", "unknown type");
}

#[test]
fn explicit_disc_default_is_first() {
	check("enum E { a = 5, b c }\nx: E\nx", "a");
}

#[test]
fn atom_coerces_in_annotated_binding() {
	check("enum Color { red green blue }\nc : Color : :blue\nc", "blue");
}

#[test]
fn atom_coerces_in_assignment() {
	check(
		indoc! {"
			enum Color { red green blue }
			c := Color.green
			c = :red
			c
		"},
		"red",
	);
}

#[test]
fn atom_coerces_in_comparison() {
	check("enum Color { red green blue }\nc :: Color.red\nc == :red", "true");
	check("enum Color { red green blue }\nColor.blue == :blue", "true");
}

#[test]
fn atom_coerces_in_struct_field() {
	check(
		indoc! {"
			enum Stat { health mana stamina }
			struct User { s Stat }
			u :: User{ s: :mana }
			u.s
		"},
		"mana",
	);
}

#[test]
fn atom_unknown_variant() {
	fail_with(
		"enum Color { red green blue }\nc : Color : :purple",
		"no variant `purple`",
	);
}

#[test]
fn cast_to_int() {
	check("enum Color { red green blue }\nint(Color.blue)", "2");
}

#[test]
fn cast_to_int_explicit_disc() {
	check("enum Status { ok = 200, err = 500 }\nint(Status.err)", "500");
}

#[test]
fn backed_cast_to_backing() {
	check("enum Status: u8 { ok = 200, err = 250 }\nu8(Status.ok)", "200");
}

#[test]
fn backing_non_integer_errors() {
	fail_with("enum E: bool { a }", "not an enum-able type");
}

#[test]
fn backing_out_of_range_errors() {
	fail_with("enum E: u8 { a = 300 }", "out of range for its backing type");
}

#[test]
fn backing_with_payload_errors() {
	fail_with("enum E: u8 { a some(int) }", "cannot have payload");
}

#[test]
fn compare_via_int() {
	check("enum Color { red green blue }\nint(Color.green) == 1", "true");
}

#[test]
fn ord_gives_discriminant() {
	check("enum Color { red green blue }\nord(Color.blue)", "2");
}

#[test]
fn ord_on_payload_variant() {
	check("enum Opt { nope some(int) }\nord(Opt.some(1))", "1");
}

#[test]
fn str_method() {
	check("enum Color { red green blue }\nColor.blue.str()", "blue");
}

#[test]
fn str_method_concat() {
	check(
		r#"enum Color { red green blue }
		"the color is " + Color.green.str()"#,
		"the color is green",
	);
}

#[test]
fn no_such_method() {
	fail_with("enum Color { red green blue }\nColor.red.hex()", "has no method `hex`");
}

#[test]
fn from_int_match() {
	check("enum Color { red green blue }\nColor.from(1) or { Color.red }", "green");
}

#[test]
fn from_int_no_match() {
	check("enum Color { red green blue }\nColor.from(9) or { Color.red }", "red");
}

#[test]
fn from_int_no_match_carries_error() {
	check(
		"enum Color { red green blue }\nColor.from(9) or { print($)\nColor.red }",
		["no matching variant", "red"],
	);
}

#[test]
fn from_str_match() {
	check(
		"enum Color { red green blue }\nColor.from(\"blue\") or { Color.red }",
		"blue",
	);
}

#[test]
fn from_str_no_match() {
	check(
		"enum Color { red green blue }\nColor.from(\"purple\") or { print($)\nColor.red }",
		"no matching variant\nred",
	);
}

#[test]
fn from_atom_match() {
	check(
		"enum Color { red green blue }\nColor.from(:blue) or { Color.red }",
		"blue",
	);
}

#[test]
fn from_atom_no_match() {
	check(
		"enum Color { red green blue }\nColor.from(:purple) or { print($)\nColor.red }",
		["no matching variant", "red"],
	);
}

#[test]
fn from_payload_zero_fills() {
	check(
		"enum Shape { point triangle(f64, f64, f64) }\nShape.from(1) or { Shape.point }",
		"triangle(0.0, 0.0, 0.0)",
	);
}

#[test]
fn from_wrong_type() {
	fail_with(
		"enum Color { red green blue }\nColor.from(true)",
		"needs an int, str, or atom",
	);
}

#[test]
fn shorthand_coerces_in_fn_arg() {
	check(
		indoc! {"
			enum Color { red green blue }
			fn name(c Color) { c.str() }
			name(.blue)
		"},
		"blue",
	);
}

#[test]
fn atom_coerces_in_fn_arg() {
	check(
		indoc! {"
			enum Color { red green blue }
			fn name(c Color) { c.str() }
			name(:blue)
		"},
		"blue",
	);
}

#[test]
fn shorthand_coerces_in_if_tail_return() {
	check(
		indoc! {"
			enum Color { red green blue }
			fn fav(pick bool) Color {
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
			enum Color { red green blue }
			fn fav(n int) Color {
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
			enum Color { red green blue }
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
			enum Color { red green blue }
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
			enum Shape {
				point triangle(f64, f64, f64)
				circle { radius f64 }
			}
			print(Shape.triangle(3.0, 4.0, 5.0))
			print(Shape.circle { radius: 5.0 })
			print(Shape.point)
		"},
		["triangle(3.0, 4.0, 5.0)", "circle{radius: 5.0}", "point"],
	);
}

#[test]
fn backed_arrays_pack() {
	check(
		indoc! {"
			enum Status: u8 { ok = 200, err = 250 }
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
			enum Suit: string { hearts = "♥" spades = "♠" }
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
	check("enum S: string { a b }\nstr(S.b)", "b");
}

#[test]
fn string_backed_errors() {
	fail_with("enum S: string { a b }\nint(S.a)", "cannot cast str");
	fail_with("enum S { a = \"x\" }", "needs a string backing");
	fail_with("enum S: string { a = 2 }", "uses raw values");
	fail_with("enum S: string { a = \"x\" b = \"x\" }", "assigned more than once");
	fail_with("enum S: string { a b = \"a\" }", "assigned more than once");
}

#[test]
fn backed_array_signed_sextends() {
	check(
		indoc! {r#"
			enum Delta: i8 { down = -3, up = 4 }
			a :: [Delta.up, Delta.down]
			d :: a[1]
			print(int(d))
			print(match d { Delta.down => "yes", else => "no" })
			d == Delta.down
		"#},
		["-3", "yes", "true"],
	);
}
