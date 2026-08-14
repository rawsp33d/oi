use crate::helpers::*;

const ANIMAL_DOG: &str = indoc! {r#"
	Animal :: trait { speak : fn(self) string }
	Dog :: struct {}
	Dog : Animal { speak :: fn(self) string { "woof" } }
"#};

const ANIMAL_DOG_KIND: &str = indoc! {r#"
	Animal :: trait { speak : fn(self) string }
	Dog :: struct { kind: string }
	Dog : Animal { speak :: fn(self) string { "woof" } }
"#};

const ANIMAL_KIND: &str = indoc! {r#"
	Animal :: trait {
		kind: string
		speak : fn(self) string
	}
	Dog :: struct { kind: string }
	Dog : Animal { speak :: fn(self) string { "woof" } }
"#};

#[test]
fn trait_def_and_impl() {
	let src = indoc! {r#"
		Animal :: trait {
			kind: string
			speak : fn(self) string
			shout :: fn(self) string { self.speak() + "!" }
		}
		Dog :: struct { kind: string }
		Dog : Animal { speak :: fn(self) string { "woof" } }
		Dog.{ "Collie" }.speak()
	"#};
	check(src, "woof");
}

#[test]
fn marker_impl() {
	let src = indoc! {r#"
		Copy :: trait {}
		Dog :: struct { kind: string }
		Dog : Copy
		Dog.{ "Collie" }.kind
	"#};
	check(src, "Collie");
}

#[test]
fn lowercase_claim_beside_zeroed_decl() {
	let src = indoc! {"
		animal :: trait {}
		dog :: struct {}
		dog : animal
		n : int
		print(dog is animal, n)
	"};
	check(src, "true 0");
}

#[test]
fn supertraits() {
	let src = indoc! {"
		Eq :: trait {}
		Ord : Eq : trait { cmp : fn(self, other: Self) int }
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
		X : Eq
		X : Ord
		print(X is Ord)
	"};
	check(src, "true");

	fail(indoc! {"
		Eq :: trait {}
		Ord :: trait is Eq {}
		X :: struct {}
		X : Ord
	"});
}

#[test]
fn default_methods() {
	let src = indoc! {r#"
		Animal :: trait {
			speak : fn(self) string
			shout :: fn(self) string { self.speak() + "!" }
		}
		Dog :: struct {}
		Dog : Animal { speak :: fn(self) string { "woof" } }
		Dog.{}.shout()
	"#};
	check(src, "woof!");
	let src = indoc! {r#"
		Animal :: trait {
			speak : fn(self) string
			shout :: fn(self) string { self.speak() + "!" }
		}
		Dog :: struct {}
		Dog : Animal {
			speak :: fn(self) string { "woof" }
			shout :: fn(self) string { "WOOF" }
		}
		Dog.{}.shout()
	"#};
	check(src, "WOOF");
}

#[test]
fn field_requirement_satisfied() {
	let src = indoc! {r#"
		Animal :: trait { kind: string }
		Dog :: struct { kind: string }
		Dog : Animal
		Dog.{ "Collie" }.kind
	"#};
	check(src, "Collie");
}

#[test]
fn is_expression() {
	let src = indoc! {"
		Animal :: trait {}
		Dog :: struct {}
		Dog : Animal
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
		Dog : Animal { speak :: fn(self) string { "woof" } }
	"#});
	fail(indoc! {r#"
		Animal :: trait { speak: fn(self) string }
		Dog :: struct {}
		Dog : Animal {}
	"#});
	fail(indoc! {r#"
		Animal :: trait { kind: string }
		Dog :: struct {}
		Dog : Animal
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
fn allows_extra_helper_fills() {
	let src = indoc! {r#"
		Animal :: trait { speak: fn(self) string }
		Dog :: struct {}
		Dog : Animal {
			speak :: fn(self) string { self.noise() }
			noise :: fn(self) string { "woof" }
		}
		Dog.{}.speak()
	"#};
	check(src, "woof");
}

#[test]
fn rejects_wrong_arity_impl() {
	fail_with(
		indoc! {r#"
			Animal :: trait { speak: fn(self) string }
			Dog :: struct {}
			Dog : Animal { speak :: fn(self, loud: bool) string { "woof" } }
		"#},
		"wrong signature",
	);
}

#[test]
fn rejects_wrong_param_type_impl() {
	fail_with(
		indoc! {r#"
			Animal :: trait { speak: fn(self, times: int) string }
			Dog :: struct {}
			Dog : Animal { speak :: fn(self, times: float) string { "woof" } }
		"#},
		"wrong signature",
	);
}

#[test]
fn rejects_wrong_return_type_impl() {
	fail_with(
		indoc! {r#"
			Animal :: trait { speak: fn(self) string }
			Dog :: struct {}
			Dog : Animal { speak :: fn(self) int { 0 } }
		"#},
		"wrong signature",
	);
}

#[test]
fn one_fill_satisfies_two_traits() {
	let src = indoc! {"
		A :: trait { f: fn(self) int }
		B :: trait { f: fn(self) int }
		S :: struct {}
		S : A, B { f :: fn(self) int { 9 } }
		S.{}.f()
	"};
	check(src, "9");
}

#[test]
fn rejects_shared_fill_sig_mismatch() {
	fail_with(
		indoc! {"
			A :: trait { f: fn(self) int }
			B :: trait { f: fn(self) string }
			S :: struct {}
			S : A, B { f :: fn(self) int { 9 } }
		"},
		"declares",
	);
}

#[test]
fn rejects_conflicting_defaults() {
	fail_with(
		indoc! {"
			A :: trait { f :: fn(self) int { 1 } }
			B :: trait { f :: fn(self) int { 2 } }
			S :: struct {}
			S : A, B
		"},
		"takes default `f` from both",
	);
}

#[test]
fn own_fill_settles_default_conflict() {
	let src = indoc! {"
		A :: trait { f :: fn(self) int { 1 } }
		B :: trait { f :: fn(self) int { 2 } }
		S :: struct { f :: fn(self) int { 3 } }
		S : A, B
		S.{}.f()
	"};
	check(src, "3");
}

#[test]
fn body_fill_discharges_bare_claim() {
	let src = indoc! {"
		A :: trait { f: fn(self) int }
		S :: struct { f :: fn(self) int { 7 } }
		S : A
		S.{}.f()
	"};
	check(src, "7");
}

#[test]
fn rejects_duplicate_fill() {
	fail_with(
		indoc! {"
			S :: struct { f :: fn(self) int { 1 } }
			S :{ f :: fn(self) int { 2 } }
		"},
		"duplicate fill",
	);
}

#[test]
fn overrides_default_method_multi_param() {
	let src = indoc! {r#"
		Animal :: trait {
			speak : fn(self, times: int, loud: bool) string
			shout :: fn(self) string { self.speak(1, false) + "!" }
		}
		Dog :: struct {}
		Dog : Animal {
			speak :: fn(self, times: int, loud: bool) string { if loud { "WOOF" } else { "woof" } }
		}
		Dog.{}.speak(2, true)
	"#};
	check(src, "WOOF");
}

#[test]
fn dyn_dispatch_zoo() {
	let src = indoc! {r#"
		Cat :: struct { legs: int, kind: string }
		Cat : Animal { speak :: fn(self) string { "meow" } }
		zoo : []Animal : [ Dog.{ "collie" }, Cat.{ 4, "mau" } ]
		loop a in zoo { print("a " + a.kind + " says " + a.speak()) }
	"#};
	check([ANIMAL_KIND, src], ["a collie says woof", "a mau says meow"]);
}

#[test]
fn trait_object_array_literal() {
	let src = indoc! {r#"
		Cat :: struct { kind: string }
		Cat : Animal { speak :: fn(self) string { "meow" } }
		animals :: Animal.[ Dog.{ "collie" }, Cat.{ "mau" } ]
		loop a in animals { print("{a.kind}: {a.speak()}") }
	"#};
	check([ANIMAL_KIND, src], ["collie: woof", "mau: meow"]);
}

#[test]
fn dyn_trait_param_and_default() {
	let src = indoc! {r#"
		Animal :: trait {
			speak : fn(self) string
			shout :: fn(self) string { self.speak() + "!" }
		}
		Dog :: struct {}
		Cat :: struct {}
		Dog : Animal { speak :: fn(self) string { "woof" } }
		Cat : Animal { speak :: fn(self) string { "meow" } }
		greet :: fn(a: Animal) string { a.shout() }
		relay :: fn(a: Animal) string { greet(a) }
		print(greet(Dog.{}))
		print(relay(Cat.{}))
	"#};
	check(src, ["woof!", "meow!"]);
}

#[test]
fn trait_typed_struct_field() {
	let src = indoc! {r#"
		Pen :: struct { pet: Animal }
		p :: Pen.{ pet = Dog.{} }
		print(p.pet.speak())
	"#};
	check([ANIMAL_DOG, src], "woof");
}

#[test]
fn self_sig_static_dispatch_ok() {
	let src = indoc! {r#"
		Cloner :: trait { dup : fn(self) Self }
		Dog :: struct {}
		Dog : Cloner { dup :: fn(self) Self { Dog.{} } }
		d :: Dog.{}.dup()
		print("cloned")
	"#};
	check(src, "cloned");
}

#[test]
fn rejects_non_object_safe_trait() {
	fail(indoc! {r#"
		Cloner :: trait { dup : fn(self) Self }
		Dog :: struct {}
		Dog : Cloner { dup :: fn(self) Self { Dog.{} } }
		f :: fn(c: Cloner) string { "no" }
	"#});
	fail(indoc! {r#"
		Eater :: trait { eat : fn(self, other: Self) string }
		Dog :: struct {}
		Dog : Eater { eat :: fn(self, other: Self) string { "ate" } }
		pack :: Eater.[ Dog.{} ]
	"#});
}

#[test]
fn trait_object_renders_concrete_struct() {
	let src = indoc! {r#"
		a : Animal : Dog.{ "collie" }
		print(a)
		print("says {a}")
		print(a.str())
	"#};
	check(
		[ANIMAL_DOG_KIND, src],
		[
			r#"Dog.{kind = "collie"}"#,
			r#"says Dog.{kind = "collie"}"#,
			r#"Dog.{kind = "collie"}"#,
		],
	);
}

#[test]
fn trait_object_uses_str_override() {
	let src = indoc! {r#"
		Dog :{ str :: fn(self) string { "a " + self.kind + " dog" } }
		a : Animal : Dog.{ "collie" }
		print(a)
	"#};
	check([ANIMAL_DOG_KIND, src], "a collie dog");
}

#[test]
fn array_of_trait_objects_renders() {
	let src = indoc! {r#"
		Cat :: struct { kind: string }
		Cat : Animal { speak :: fn(self) string { "meow" } }
		print(Animal.[ Dog.{ "collie" }, Cat.{ "mau" } ])
	"#};
	check(
		[ANIMAL_DOG_KIND, src],
		"[Dog.{kind = \"collie\"}, Cat.{kind = \"mau\"}]",
	);
}

#[test]
fn trait_declared_str_dyn_dispatches() {
	let src = indoc! {r#"
		Animal :: trait { str : fn(self) string }
		Dog :: struct { kind: string }
		Dog : Animal { str :: fn(self) string { "custom-" + self.kind } }
		a : Animal : Dog.{ "collie" }
		print(a)
		print(a.str())
	"#};
	check(src, "custom-collie\ncustom-collie");
}

#[test]
fn fixed_trait_array_loops_and_dispatches() {
	let src = indoc! {r#"
		Cat :: struct { kind: string }
		Cat : Animal { speak :: fn(self) string { "meow" } }
		zoo := Animal.[ Dog.{ "collie" }, Cat.{ "mau" } ]
		loop a in zoo { print("a " + a.kind + " says " + a.speak()) }
	"#};
	check([ANIMAL_KIND, src], ["a collie says woof", "a mau says meow"]);
}

#[test]
fn append_boxes_struct_literal_into_trait_array() {
	let src = indoc! {r#"
		Cat :: struct {}
		Cat : Animal { speak :: fn(self) string { "meow" } }
		zoo: []Animal = [ Dog.{} ]
		zoo << Cat.{}
		loop a in zoo { print(a.speak()) }
	"#};
	check([ANIMAL_DOG, src], ["woof", "meow"]);
}

#[test]
fn rejects_non_implementing_trait_object() {
	let src = indoc! {"
		Rock :: struct {}
		a : Animal : Rock.{}
	"};
	fail([ANIMAL_DOG, src]);
}
