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

# $out
passthru :: fn(a: int, b: int) (int, int) {
	$out.0 = a
	$out.1 = b
	return
}

# $out
passthru :: fn(a: int, b: int) out (c: int, d: int) {
	out.c = a
	out.d = b
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
```
