use crate::helpers::*;

#[test]
fn trait_def_and_impl() {
	let src = indoc! {r#"
		trait Animal {
			kind string
			fn speak(self) string
			fn shout(self) string { self.speak() + "!" }
		}
		struct Dog { kind string }
		impl Animal for Dog { fn speak(self) string { "woof" } }
		Dog{ "Collie" }.speak()
	"#};
	check(src, "woof");
}

#[test]
fn marker_impl() {
	let src = indoc! {r#"
		trait Copy {}
		struct Dog { kind string }
		impl Copy for Dog
		Dog{ "Collie" }.kind
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

#[test]
fn default_methods() {
	let src = indoc! {r#"
		trait Animal {
			fn speak(self) string
			fn shout(self) string { self.speak() + "!" }
		}
		struct Dog {}
		impl Animal for Dog { fn speak(self) string { "woof" } }
		Dog{}.shout()
	"#};
	check(src, "woof!");
	let src = indoc! {r#"
		trait Animal {
			fn speak(self) string
			fn shout(self) string { self.speak() + "!" }
		}
		struct Dog {}
		impl Animal for Dog {
			fn speak(self) string { "woof" }
			fn shout(self) string { "WOOF" }
		}
		Dog{}.shout()
	"#};
	check(src, "WOOF");
}

#[test]
fn field_requirement_satisfied() {
	let src = indoc! {r#"
		trait Animal { kind string }
		struct Dog { kind string }
		impl Animal for Dog
		Dog{ "Collie" }.kind
	"#};
	check(src, "Collie");
}

#[test]
fn rejects_bad_impls() {
	fail(indoc! {r#"
		struct Dog {}
		impl Animal for Dog { fn speak(self) string { "woof" } }
	"#});
	fail(indoc! {r#"
		trait Animal { fn speak(self) string }
		struct Dog {}
		impl Animal for Dog {}
	"#});
	fail(indoc! {r#"
		trait Animal { kind string }
		struct Dog {}
		impl Animal for Dog
	"#});
}
