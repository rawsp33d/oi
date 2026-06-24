use crate::helpers::*;

#[test]
fn num_seperators() {
	check("1_000_000_000", "1000000000");
	check("1_2_3_4_5", "12345");
	check("10_000.22", "10000.22");
	// TODO: add back once I add binary/octal/hex numbers
	// check("0b1_1111_1111", "0b111111111");
	// check("0o7_5_5", "0o755");
	// check("0xFF80_0000_0000_0000", "0xFF80000000000000");
}
