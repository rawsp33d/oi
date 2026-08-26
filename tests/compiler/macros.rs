use crate::common::Project;
use crate::helpers::*;

#[test]
fn dbg_passes_value_through() {
	let (out, err) = run_streams("dbg!(1 + 2)");
	assert_eq!(out.trim(), "3");
	assert!(err.contains("1 + 2 = 3"), "stderr:\n{err}");
}

#[test]
fn assert_statement_form() {
	check("assert! 1 + 1 == 2", "");
	fail_with("assert! 1 == 2", "assertion failed: 1 == 2");
}

#[test]
fn helpers_abort() {
	fail_with("todo!()", "not yet implemented");
	fail_with("unreachable!()", "entered unreachable code");
	fail_with(r#"todo!("idk")"#, "idk");
}

#[test]
fn unknown_macro_errors() {
	fail_with("nope!(1)", "no macro named");
}

#[test]
fn module_fn_body_uses_a_sibling_macro() {
	Project::new()
		.file("main.oi", ["module main", "use util", "print(util.f())"])
		.file(
			"util/lib.oi",
			[
				"module util",
				"pub say! :: fn(n: Ast) Ast { `%n * 2` }",
				"pub f :: fn() int { say!(21) }",
			],
		)
		.check("42");
}

#[test]
fn macro_run_error_is_a_diagnostic() {
	let src = indoc! {r"
		grow! :: fn(n: Ast) Ast { `%{n.int() + 1}` }
		print(grow!(true))
	"};
	fail_with(src, "while running `grow!`");
}

#[test]
fn bare_call_suggests_macro() {
	fail_with("assert(true)", "write `assert!(...)`");
}

#[test]
fn template_macro_in_expr_position() {
	let src = indoc! {"
		twice! :: fn(x: Ast) Ast { `%x + %x` }
		twice!(4)
	"};
	check(src, "8");
}

#[test]
fn template_macro_is_hygienic() {
	let src = indoc! {"
		set_and_add! :: fn(n: Ast) Ast {
			`tmp := 100
			tmp + %n`
		}
		tmp := 5
		result := set_and_add!(tmp)
		tmp + result
	"};
	check(src, "110");
}

#[test]
fn binder_unquote_deliberately_captures() {
	let src = indoc! {"
		setup! :: fn(n: Ast) Ast { `%n := 42` }
		setup!(x)
		x
	"};
	check(src, "42");
}

#[test]
fn template_macro_statement_form() {
	let src = indoc! {"
		incr! :: fn(n: Ast) Ast { `%n + 1` }
		incr! 4
	"};
	check(src, "5");
}

#[test]
fn template_macro_wrong_arity_fails() {
	let src = indoc! {"
		twice! :: fn(x: Ast) Ast { `%x + %x` }
		twice!(1, 2)
	"};
	fail_with(src, "takes 1 argument, got 2");
}

#[test]
fn macro_expands_to_an_item() {
	let src = indoc! {"
		shape! :: fn() Ast {
			`Point :: struct { x: int, y: int }`
		}
		shape!()
		p := Point.{ x = 1, y = 2 }
		p.x + p.y
	"};
	check(src, "3");
}

#[test]
fn comptime_loop_builds_ast() {
	check(
		indoc! {r"
			unroll! :: fn(n: Ast, body: Ast) Ast {
				acc := `0`
				i := 0
				loop i < n.int() {
					acc = `%acc + %body`
					i = i + 1
				}
				acc
			}
			print(unroll!(3, 5))
		"},
		"15",
	);
}

#[test]
fn comptime_recursion_through_quotes() {
	check(
		indoc! {r"
			tri! :: fn(n: Ast) Ast {
				m := n.int()
				if m <= 1 { `1` } else {
					k := m - 1
					`%n + tri!(%k)`
				}
			}
			print(tri!(4))
		"},
		"10",
	);
}

#[test]
fn macros_call_macros_directly() {
	check(
		indoc! {r"
			one! :: fn() Ast { `1` }
			wrap! :: fn(x: Ast) Ast {
				y := one!()
				`%x + %y`
			}
			print(wrap!(4))
		"},
		"5",
	);
}

#[test]
fn macro_ret_must_be_ast() {
	let src = indoc! {"
		bad! :: fn(x: Ast) int { 1 }
		bad!(1)
	"};
	fail_with(src, "macros return `Ast`");
}

#[test]
fn unquote_lifts_primitives() {
	check(
		indoc! {r#"
			mk! :: fn() Ast {
				s := "hi"
				b := true
				f := 2.5
				`if %b {
					print(%s)
					print(%f)
				}`
			}
			mk!()
		"#},
		["hi", "2.5"],
	);
}

#[test]
fn unquote_expr_splices_int_result() {
	check(
		indoc! {r"
			xten! :: fn(x: Ast) Ast { `%x + %{x.int() * 10}` }
			print(xten!(4))
		"},
		"44",
	);
}

#[test]
fn unquote_expr_nesting() {
	check(
		indoc! {r"
			dub! :: fn(x: Ast) Ast { `1 + %{`%x * 2`}` }
			print(dub!(3))
		"},
		"7",
	);
}

#[test]
fn splat_spreads_into_call_args() {
	check(
		indoc! {r"
			add3 :: fn(a: int, b: int, c: int) int { a + b + c }
			sum! :: fn(xs: Ast) Ast { `add3(%{...xs.items})` }
			print(sum!([1, 2, 3]))
		"},
		"6",
	);
}

#[test]
fn splat_spreads_into_statements() {
	check(
		indoc! {r"
			noisy! :: fn() Ast {
				xs := [`print(1)`, `print(2)`]
				`%{...xs}`
			}
			noisy!()
			print(3)
		"},
		["1", "2", "3"],
	);
}

#[test]
fn ident_compares_with_str() {
	check(
		indoc! {r#"
			pick! :: fn(t: Ast) Ast { if t == "Hash" { `1` } else { `2` } }
			print(pick!(Hash))
			print(pick!(Debug))
		"#},
		["1", "2"],
	);
}

#[test]
fn name_reads_a_def() {
	check(
		indoc! {r#"
			shape! :: fn() Ast {
				s := `P :: struct { x: int }`
				if s.name == "P" { `1` } else { `0` }
			}
			print(shape!())
		"#},
		"1",
	);
}

#[test]
fn items_list_struct_fields() {
	check(
		indoc! {r"
			shape! :: fn() Ast {
				s := `Pt :: struct { x: int, y: int, z: int }`
				fs := s.items
				`%{fs.len}`
			}
			print(shape!())
		"},
		"3",
	);
}

#[test]
fn macro_body_calls_an_imported_fn() {
	check(
		indoc! {r"
			use math
			five! :: fn() Ast {
				v := math.abs(0 - 5)
				`%v`
			}
			print(five!())
		"},
		"5",
	);
}

#[test]
fn qualified_macro_call_resolves_module_locally() {
	Project::new()
		.file("main.oi", ["module main", "use util", "print(util.answer!())"])
		.file(
			"util/lib.oi",
			[
				"module util",
				"helper :: fn() int { 40 }",
				"pub answer! :: fn() Ast {",
				"v := helper() + 2",
				"`%v`",
				"}",
			],
		)
		.check("42");
}

#[test]
fn qualified_macro_statement_form() {
	Project::new()
		.file("main.oi", ["module main", "use util", "util.say! 6"])
		.file(
			"util/lib.oi",
			["module util", "pub say! :: fn(n: Ast) Ast { `print(%n)` }"],
		)
		.check("6");
}

#[test]
fn imported_macro_called_bare() {
	Project::new()
		.file("main.oi", ["module main", "use util.{ answer }", "print(answer!())"])
		.file("util/lib.oi", ["module util", "pub answer! :: fn() Ast { `42` }"])
		.check("42");
}
