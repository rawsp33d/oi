use crate::helpers::*;

#[test]
fn trait_def_and_impl() {
	let src = indoc! {r#"
		trait Animal {
			kind string
			fn speak(self) string
			fn shout(self) string { self.speak().upper() }
		}
		struct Dog { kind string }
		impl Animal for Dog { fn speak(self) string { "woof" } }
		Dog{"Collie"}.speak()
	"#};
	check(src, "woof");
}

#[test]
fn marker_impl() {
	let src = indoc! {r#"
		trait Copy {}
		struct Dog { kind string }
		impl Copy for Dog
		Dog{"Collie"}.kind
	"#};
	check(src, "Collie");
}

#[test]
fn supertraits() {
	let src = indoc! {"
		trait Eq {}
		trait Ord is Eq { fn cmp(self, other Self) int }
	"};
	check(src, "");

	let src = indoc! {"
		trait Foo {}
		trait Baz is Foo, Bar {}
	"};
	check(src, "");
}
