use crate::helpers::*;

const ANIMAL_DOG: &str = indoc! {r#"
	Animal :: trait { fn speak(self) string }
	Dog :: struct {}
	impl Animal for Dog { speak :: fn(self) string { "woof" } }
"#};

const ANIMAL_DOG_KIND: &str = indoc! {r#"
	Animal :: trait { fn speak(self) string }
	Dog :: struct { kind: string }
	impl Animal for Dog { speak :: fn(self) string { "woof" } }
"#};

const ANIMAL_KIND: &str = indoc! {r#"
	Animal :: trait {
		kind: string
		fn speak(self) string
	}
	Dog :: struct { kind: string }
	impl Animal for Dog { speak :: fn(self) string { "woof" } }
"#};

#[test]
fn trait_def_and_impl() {
	let src = indoc! {r#"
		Animal :: trait {
			kind: string
			fn speak(self) string
			fn shout(self) string { self.speak() + "!" }
		}
		Dog :: struct { kind: string }
		impl Animal for Dog { speak :: fn(self) string { "woof" } }
		Dog{ "Collie" }.speak()
	"#};
	check(src, "woof");
}

#[test]
fn marker_impl() {
	let src = indoc! {r#"
		Copy :: trait {}
		Dog :: struct { kind: string }
		impl Copy for Dog
		Dog{ "Collie" }.kind
	"#};
	check(src, "Collie");
}

#[test]
fn supertraits() {
	let src = indoc! {"
		Eq :: trait {}
		Ord : Eq : trait { fn cmp(self, other: Self) int }
	"};
	check(src, "");

	let src = indoc! {"
		Foo :: trait {}
		Baz : Foo, Bar : trait {}
	"};
	check(src, "");

	let src = indoc! {"
		Eq :: trait {}
		Ord : Eq : trait {}
		X :: struct {}
		impl Eq for X
		impl Ord for X
		print(X is Ord)
	"};
	check(src, "true");

	fail(indoc! {"
		Eq :: trait {}
		Ord :: trait is Eq {}
		X :: struct {}
		impl Ord for X
	"});
}

#[test]
fn default_methods() {
	let src = indoc! {r#"
		Animal :: trait {
			fn speak(self) string
			fn shout(self) string { self.speak() + "!" }
		}
		Dog :: struct {}
		impl Animal for Dog { speak :: fn(self) string { "woof" } }
		Dog{}.shout()
	"#};
	check(src, "woof!");
	let src = indoc! {r#"
		Animal :: trait {
			fn speak(self) string
			fn shout(self) string { self.speak() + "!" }
		}
		Dog :: struct {}
		impl Animal for Dog {
			speak :: fn(self) string { "woof" }
			shout :: fn(self) string { "WOOF" }
		}
		Dog{}.shout()
	"#};
	check(src, "WOOF");
}

#[test]
fn field_requirement_satisfied() {
	let src = indoc! {r#"
		Animal :: trait { kind: string }
		Dog :: struct { kind: string }
		impl Animal for Dog
		Dog{ "Collie" }.kind
	"#};
	check(src, "Collie");
}

#[test]
fn is_expression() {
	let src = indoc! {"
		Animal :: trait {}
		Dog :: struct {}
		impl Animal for Dog
		D :: Dog
		print(Dog is Animal)
		print(Dog is not Animal)
		print(D is Animal)
	"};
	check(src, ["true", "false", "true"]);

	let src = indoc! {"
		Animal :: trait {}
		Cat :: struct {}
		print(Cat is Animal)
		print(Cat is not Animal)
	"};
	check(src, ["false", "true"]);
}

#[test]
fn is_expression_unknown_type() {
	fail(indoc! {"
		Animal :: trait {}
		print(Ghost is Animal)
	"});
}

#[test]
fn rejects_bad_impls() {
	fail(indoc! {r#"
		Dog :: struct {}
		impl Animal for Dog { speak :: fn(self) string { "woof" } }
	"#});
	fail(indoc! {r#"
		Animal :: trait { speak :: fn(self) string }
		Dog :: struct {}
		impl Animal for Dog {}
	"#});
	fail(indoc! {r#"
		Animal :: trait { kind: string }
		Dog :: struct {}
		impl Animal for Dog
	"#});
}

#[test]
fn rejects_duplicate_trait() {
	fail(indoc! {"
		Animal :: trait { speak :: fn(self) string }
		Animal :: trait { bark :: fn(self) string }
	"});
}

#[test]
fn rejects_method_not_in_trait() {
	fail(indoc! {r#"
		Animal :: trait { speak :: fn(self) string }
		Dog :: struct {}
		impl Animal for Dog {
			speak :: fn(self) string { "woof" }
			fetch :: fn(self) string { "stick" }
		}
	"#});
}

#[test]
fn rejects_wrong_arity_impl() {
	fail(indoc! {r#"
		Animal :: trait { speak :: fn(self) string }
		Dog :: struct {}
		impl Animal for Dog { speak :: fn(self, loud: bool) string { "woof" } }
	"#});
}

#[test]
fn rejects_wrong_param_type_impl() {
	fail(indoc! {r#"
		Animal :: trait { speak :: fn(self, times: int) string }
		Dog :: struct {}
		impl Animal for Dog { speak :: fn(self, times: float) string { "woof" } }
	"#});
}

#[test]
fn rejects_wrong_return_type_impl() {
	fail(indoc! {r#"
		Animal :: trait { speak :: fn(self) string }
		Dog :: struct {}
		impl Animal for Dog { speak :: fn(self) int { 0 } }
	"#});
}

#[test]
fn overrides_default_method_multi_param() {
	let src = indoc! {r#"
		Animal :: trait {
			fn speak(self, times: int, loud: bool) string
			fn shout(self) string { self.speak(1, false) + "!" }
		}
		Dog :: struct {}
		impl Animal for Dog {
			speak :: fn(self, times: int, loud: bool) string { if loud { "WOOF" } else { "woof" } }
		}
		Dog{}.speak(2, true)
	"#};
	check(src, "WOOF");
}

#[test]
fn dyn_dispatch_zoo() {
	let src = indoc! {r#"
		Cat :: struct { legs: int, kind: string }
		impl Animal for Cat { speak :: fn(self) string { "meow" } }
		zoo : []Animal : [ Dog{ "collie" }, Cat{ 4, "mau" } ]
		loop a in zoo { print("a " + a.kind + " says " + a.speak()) }
	"#};
	check([ANIMAL_KIND, src], ["a collie says woof", "a mau says meow"]);
}

#[test]
fn trait_object_array_literal() {
	let src = indoc! {r#"
		Cat :: struct { kind: string }
		impl Animal for Cat { speak :: fn(self) string { "meow" } }
		animals :: []Animal{ Dog{ "collie" }, Cat{ "mau" } }
		loop a in animals { print("{a.kind}: {a.speak()}") }
	"#};
	check([ANIMAL_KIND, src], ["collie: woof", "mau: meow"]);
}

#[test]
fn dyn_trait_param_and_default() {
	let src = indoc! {r#"
		Animal :: trait {
			fn speak(self) string
			fn shout(self) string { self.speak() + "!" }
		}
		Dog :: struct {}
		Cat :: struct {}
		impl Animal for Dog { speak :: fn(self) string { "woof" } }
		impl Animal for Cat { speak :: fn(self) string { "meow" } }
		greet :: fn(a: Animal) string { a.shout() }
		relay :: fn(a: Animal) string { greet(a) }
		print(greet(Dog{}))
		print(relay(Cat{}))
	"#};
	check(src, ["woof!", "meow!"]);
}

#[test]
fn trait_typed_struct_field() {
	let src = indoc! {r#"
		Pen :: struct { pet: Animal }
		p :: Pen{ pet: Dog{} }
		print(p.pet.speak())
	"#};
	check([ANIMAL_DOG, src], "woof");
}

#[test]
fn self_sig_static_dispatch_ok() {
	let src = indoc! {r#"
		Cloner :: trait { fn dup(self) Self }
		Dog :: struct {}
		impl Cloner for Dog { dup :: fn(self) Self { Dog{} } }
		d :: Dog{}.dup()
		print("cloned")
	"#};
	check(src, "cloned");
}

#[test]
fn rejects_non_object_safe_trait() {
	fail(indoc! {r#"
		Cloner :: trait { fn dup(self) Self }
		Dog :: struct {}
		impl Cloner for Dog { dup :: fn(self) Self { Dog{} } }
		f :: fn(c: Cloner) string { "no" }
	"#});
	fail(indoc! {r#"
		Eater :: trait { fn eat(self, other: Self) string }
		Dog :: struct {}
		impl Eater for Dog { eat :: fn(self, other: Self) string { "ate" } }
		pack :: []Eater{ Dog{} }
	"#});
}

#[test]
fn trait_object_renders_concrete_struct() {
	let src = indoc! {r#"
		a : Animal : Dog{ "collie" }
		print(a)
		print("says {a}")
		print(a.str())
	"#};
	check(
		[ANIMAL_DOG_KIND, src],
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
		impl Dog { str :: fn(self) string { "a " + self.kind + " dog" } }
		a : Animal : Dog{ "collie" }
		print(a)
	"#};
	check([ANIMAL_DOG_KIND, src], "a collie dog");
}

#[test]
fn array_of_trait_objects_renders() {
	let src = indoc! {r#"
		Cat :: struct { kind: string }
		impl Animal for Cat { speak :: fn(self) string { "meow" } }
		print([]Animal{ Dog{ "collie" }, Cat{ "mau" } })
	"#};
	check([ANIMAL_DOG_KIND, src], "[Dog{kind: \"collie\"}, Cat{kind: \"mau\"}]");
}

#[test]
fn trait_declared_str_dyn_dispatches() {
	let src = indoc! {r#"
		Animal :: trait { fn str(self) string }
		Dog :: struct { kind: string }
		impl Animal for Dog { str :: fn(self) string { "custom-" + self.kind } }
		a : Animal : Dog{ "collie" }
		print(a)
		print(a.str())
	"#};
	check(src, "custom-collie\ncustom-collie");
}

#[test]
fn rejects_non_implementing_trait_object() {
	let src = indoc! {"
		Rock :: struct {}
		a : Animal : Rock{}
	"};
	fail([ANIMAL_DOG, src]);
}
