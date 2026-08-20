---
id: lab
aliases: []
tags: []
---
# [[Oi|../]]

Things I'm playing with that might not work or make it.

```rust
# name type args on a value with `as`, for when there's nothing to infer from
# TODO: revisit.exe
#       I was trying to take good parts of GDScript here but it feels confusing.
#       I forget the situations I thought of where you can't just put the type on LHS.
empty := Stack.{} as Stack[int]
meters := Tagged.{ value: 5.0 } as Tagged[Meters]

# normie
add :: fn(a: int, b: int) int {
	return a + b
}

# implicit return
add :: fn(a: int, b: int) int {
	a + b
}

# normie named
add :: fn(a: int, b: int) out int {
	out
}

# implicit return named
add :: fn(a: int, b: int) int {
	a + b
}

# normie tuple
passthru :: fn(a: int, b: int) (int, int) {
	return (a, b)
}

# implicit tuple
passthru :: fn(a: int, b: int) (int, int) {
	(a, b)
}

# normie multiple return
passthru :: fn(a: int, b: int) (int, int) {
	return a, b
}

# implicit multiple return
passthru :: fn(a: int, b: int) (int, int) {
	a, b
}

# named tuple
passthru :: fn(a: int, b: int) (c: int, d: int) {
	c = a
	d = b
	return
}

new_dude :: fn(name: string) Dude {
	Dude.{
		name: name
	}
}

new_dude :: fn(name: string) d Dude {
	d.name = name
	d
}

# Odin's keyed entries in fixed array literals for sparse init
codes := .[0 = "ok", 5 = "err"] # [6]string

# `.()` (Odin, Capy, Jai)
price := Money.(500)
half := f32.(1 / 2)
lime := Color.(2)

# punning with `$x -> x = x`, since just `x` had some issues and I had to take it out. idk
cfg :: Config.{ $verbose, $debug }
# it actually would make it clear enough to use in fn calls, where naked `x` would just look positional...
save(user, $overwrite)
# idk sigil, was thinking `$` but that might be confusing with Oi's implicit `$` input
# IIRC Ruby uses `@x` and OCaml uses `x~`
```
