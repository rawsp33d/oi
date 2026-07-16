default: fmt test lint

# compile and run an Oi script
[group("oi")]
exec:
	cargo run -- exec

# compile and run an Oi file
[group("oi")]
run:
	cargo run -- run

# start an interactive Oi REPL
[group("oi")]
repl:
	cargo run -- repl

# generate and serve static website
[group("docs")]
serve:
	zola --root www serve --base-url localhost

[group("cargo")]
fmt:
	cargo fmt

[group("cargo")]
lint:
	cargo clippy --all -- -D warnings

[group("cargo")]
test:
	cargo test

[group("cargo")]
[group("docs")]
doc:
	cargo doc --no-deps --verbose
