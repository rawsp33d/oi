# [[Oi|../]]

Things I'm playing with that might not work or make it.

```rust
# name type args on a value with `as`, for when there's nothing to infer from
# TODO: revisit.exe
#       I was trying to take good parts of GDScript here but it feels confusing.
#       Why not just put the type on LHS?
empty := Stack{} as Stack[int]
meters := Tagged{ value: 5.0 } as Tagged[Meters]

# normie
fn add(a int, b int) int {
	return a + b
}

# implicit return
fn add(a int, b int) int {
	a + b
}

# normie named
fn add(a int, b int) out int {
	out
}

# implicit return named
fn add(a int, b int) int {
	a + b
}

# normie tuple
fn passthru(a int, b int) (int, int) {
	return (a, b)
}

# implicit tuple
fn passthru(a int, b int) (int, int) {
	(a, b)
}

# normie multiple return
fn passthru(a int, b int) (int, int) {
	return a, b
}

# implicit multiple return
fn passthru(a int, b int) (int, int) {
	a, b
}

# named tuple
fn passthru(a int, b int) (c int, d int) {
	c = a
	d = b
	return
}

# $out
fn passthru(a int, b int) (int, int) {
	$out.0 = a
	$out.1 = b
	return
}

# $out
fn passthru(a int, b int) out (c int, d int) {
	out.c = a
	out.d = b
	return
}

fn new_dude(name string) Dude {
	Dude{
		name: name
	}
}

fn new_dude(name string) d Dude {
	d.name = name
	d
}
```