# compile and run an Oi script
[group("oi")]
@exec *args:
	cargo run --quiet -- exec "$@"

# compile and run an Oi file
[group("oi")]
@run *args:
	cargo run --quiet -- run "$@"

# run tests
[group("oi")]
@test *args:
	cargo run --quiet -- test "$@"

# start an interactive Oi REPL
[group("oi")]
[default]
@repl:
	cargo run --quiet -- repl
