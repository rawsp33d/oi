use crate::helpers::*;

#[test]
fn variable() {
	check("x := 42\nx", "42");
}
