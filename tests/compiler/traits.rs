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

	let src = indoc! {"
		trait Eq {}
		trait Ord is Eq {}
		struct X {}
		impl Eq for X
		impl Ord for X
		print(X is Ord)
	"};
	check(src, "true");

	fail(indoc! {"
		trait Eq {}
		trait Ord is Eq {}
		struct X {}
		impl Ord for X
	"});
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
fn is_expression() {
	let src = indoc! {"
		trait Animal {}
		struct Dog {}
		impl Animal for Dog
		type D = Dog
		print(Dog is Animal)
		print(Dog is not Animal)
		print(D is Animal)
	"};
	check(src, ["true", "false", "true"]);

	let src = indoc! {"
		trait Animal {}
		struct Cat {}
		print(Cat is Animal)
		print(Cat is not Animal)
	"};
	check(src, ["false", "true"]);
}

#[test]
fn is_expression_unknown_type() {
	fail(indoc! {"
		trait Animal {}
		print(Ghost is Animal)
	"});
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

#[test]
fn dyn_dispatch_zoo() {
	let src = indoc! {r#"
		trait Animal {
			kind string
			fn speak(self) string
		}
		struct Dog { kind string }
		struct Cat { legs int, kind string }
		impl Animal for Dog { fn speak(self) string { "woof" } }
		impl Animal for Cat { fn speak(self) string { "meow" } }
		zoo []Animal := [ Dog{ "collie" }, Cat{ 4, "mau" } ]
		loop a in zoo { print("a " + a.kind + " says " + a.speak()) }
	"#};
	check(src, ["a collie says woof", "a mau says meow"]);
}

#[test]
fn trait_object_array_literal() {
	let src = indoc! {r#"
		trait Animal {
			kind string
			fn speak(self) string
		}
		struct Dog { kind string }
		struct Cat { kind string }
		impl Animal for Dog { fn speak(self) string { "woof" } }
		impl Animal for Cat { fn speak(self) string { "meow" } }
		animals := []Animal{ Dog{ "collie" }, Cat{ "mau" } }
		loop a in animals { print("{a.kind}: {a.speak()}") }
	"#};
	check(src, ["collie: woof", "mau: meow"]);
}

#[test]
fn dyn_trait_param_and_default() {
	let src = indoc! {r#"
		trait Animal {
			fn speak(self) string
			fn shout(self) string { self.speak() + "!" }
		}
		struct Dog {}
		struct Cat {}
		impl Animal for Dog { fn speak(self) string { "woof" } }
		impl Animal for Cat { fn speak(self) string { "meow" } }
		fn greet(a Animal) string { a.shout() }
		fn relay(a Animal) string { greet(a) }
		print(greet(Dog{}))
		print(relay(Cat{}))
	"#};
	check(src, ["woof!", "meow!"]);
}

#[test]
fn trait_object_renders_concrete_struct() {
	let src = indoc! {r#"
		trait Animal { fn speak(self) string }
		struct Dog { kind string }
		impl Animal for Dog { fn speak(self) string { "woof" } }
		a Animal := Dog{ "collie" }
		print(a)
		print("says {a}")
		print(a.str())
	"#};
	check(
		src,
		[
			r#"Dog{kind: "collie"}"#,
			r#"says Dog{kind: "collie"}"#,
			r#"Dog{kind: "collie"}"#,
		],
	);
}

#[test]
fn trait_object_uses_str_override() {
	let src = indoc! {r#"
		trait Animal { fn speak(self) string }
		struct Dog { kind string }
		impl Dog { fn str(self) string { "a " + self.kind + " dog" } }
		impl Animal for Dog { fn speak(self) string { "woof" } }
		a Animal := Dog{ "collie" }
		print(a)
	"#};
	check(src, "a collie dog");
}

#[test]
fn array_of_trait_objects_renders() {
	let src = indoc! {r#"
		trait Animal { fn speak(self) string }
		struct Dog { kind string }
		struct Cat { kind string }
		impl Animal for Dog { fn speak(self) string { "woof" } }
		impl Animal for Cat { fn speak(self) string { "meow" } }
		print([]Animal{ Dog{ "collie" }, Cat{ "mau" } })
	"#};
	check(src, "[Dog{kind: \"collie\"}, Cat{kind: \"mau\"}]");
}

#[test]
fn trait_declared_str_dyn_dispatches() {
	let src = indoc! {r#"
		trait Animal { fn str(self) string }
		struct Dog { kind string }
		impl Animal for Dog { fn str(self) string { "custom-" + self.kind } }
		a Animal := Dog{ "collie" }
		print(a)
		print(a.str())
	"#};
	check(src, "custom-collie\ncustom-collie");
}

#[test]
fn rejects_non_implementing_trait_object() {
	fail(indoc! {r#"
		trait Animal { fn speak(self) string }
		struct Dog {}
		impl Animal for Dog { fn speak(self) string { "woof" } }
		struct Rock {}
		a Animal := Rock{}
	"#});
}
