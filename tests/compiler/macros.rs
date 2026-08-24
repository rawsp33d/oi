use crate::helpers::*;

#[test]
fn dbg_passes_value_through() {
	check("dbg!(1 + 2)", "3");
}

#[test]
fn dbg_prints_snippet_and_value_to_stderr() {
	let (_, err) = run_streams("dbg!(1 + 2)");
	assert!(err.contains("1 + 2 = 3"), "stderr:\n{err}");
}

#[test]
fn assert_statement_form() {
	check("assert! 1 + 1 == 2", "");
}

#[test]
fn assert_statement_form_fails_with_snippet() {
	fail_with("assert! 1 == 2", "assertion failed: 1 == 2");
}

#[test]
fn helpers_abort() {
	fail_with("todo!()", "not yet implemented");
	fail_with("unreachable!()", "entered unreachable code");
}

#[test]
fn helpers_with_message() {
	fail_with(r#"todo!("idk")"#, "idk");
	fail_with(r#"unreachable!("nope")"#, "nope");
}

#[test]
fn unknown_macro_errors() {
	fail_with("nope!(1)", "no macro named");
}

#[test]
fn bare_assert_call_suggests_macro() {
	fail_with("assert(true)", "write `assert!(...)`");
}

#[test]
fn bare_panic_call_suggests_macro() {
	fail_with(r#"panic("oops")"#, "write `panic!(...)`");
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
fn binder_unquote_needs_a_plain_name() {
	let src = indoc! {"
		setup! :: fn(n: Ast) Ast { `%n := 42` }
		setup!(1 + 2)
	"};
	fail_with(src, "needs a plain name argument");
}

#[test]
fn template_macro_splices_items() {
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
fn unquote_inside_anon_fn() {
	let src = indoc! {"
		apply5! :: fn(f: Ast) Ast {
			`g := fn(x: int) int { %f + x }
			g(5)`
		}
		n := 10
		apply5!(n * 2)
	"};
	check(src, "25");
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
fn unquote_expr_may_call_a_macro() {
	check(
		indoc! {r"
			two! :: fn() Ast { `2` }
			add! :: fn(x: Ast) Ast { `%x + %{two!()}` }
			print(add!(5))
		"},
		"7",
	);
}

#[test]
fn splat_spreads_into_call_args() {
	check(
		indoc! {r"
			add3 :: fn(a: int, b: int, c: int) int { a + b + c }
			sum! :: fn(xs: Ast) Ast { `add3(%{...xs.items()})` }
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
fn splat_needs_an_ast_array() {
	let src = indoc! {r"
		bad! :: fn(x: Ast) Ast { `f(%{...x.int()})` }
		bad!(1)
	"};
	fail_with(src, "expected `[]Ast`");
}
