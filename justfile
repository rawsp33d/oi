mod make

set positional-arguments

# various useful checks
[parallel]
check: build && fmt test lint

# used by the CI pipeline
ci: build fmt-check lint test

# build
[group("cargo")]
@build:
	cargo build

# run formatter
[group("cargo")]
@fmt:
	cargo fmt

# run formatter in check-only mode
[group("cargo")]
@fmt-check:
	cargo fmt --check

# run lints
[group("cargo")]
@lint:
	cargo clippy --all-targets --no-deps -- -D warnings

# run tests
[group("cargo")]
@test *args:
	cargo test "$@"

# install
[group("cargo")]
@install *args:
	cargo install --path . "$@"

# build rustdocs
[group("cargo")]
[group("docs")]
@doc:
	cargo doc --no-deps --verbose

# generate and serve static website
[group("docs")]
@serve: make::examples
	zola --root www serve --interface 0.0.0.0 --base-url /

# fix fixable things
[group("cargo")]
@fix:
	cargo fix --allow-dirty
	cargo clippy --no-deps --fix --allow-dirty

# compile and run an Oi script
[group("oi")]
@exec *args:
	cargo run --quiet -- exec "$@"

# compile and run an Oi file
[group("oi")]
@run *args:
	cargo run --quiet -- run "$@"

# start an interactive Oi REPL
[group("oi")]
[default]
@repl:
	cargo run --quiet -- repl
