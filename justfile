[parallel]
main: fmt test lint

# build
[group("cargo")]
build:
	cargo build

# run formatter
[group("cargo")]
fmt: build
	cargo fmt

# run lints
[group("cargo")]
lint: build
	cargo clippy --no-deps -- -D warnings

# run tests
[group("cargo")]
test: build
	cargo test

# build rustdocs
[group("cargo")]
[group("docs")]
doc:
	cargo doc --no-deps --verbose

# generate and serve static website
[group("docs")]
serve:
	zola --root www serve --base-url localhost

# fix fixable things
[group("cargo")]
fix:
	cargo fix --allow-dirty
	cargo clippy --no-deps --fix --allow-dirty

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
