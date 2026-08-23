use oi::Reported;
use oi::driver::run_source;

pub fn run() -> Result<(), Reported> {
	let commands = vec![
		":help".into(),
		":h".into(),
		":quit".into(),
		":q".into(),
		":exit".into(),
		":x".into(),
		":clear".into(),
		":c".into(),
	];

	let mut rl = reedline::Reedline::create()
		.with_edit_mode(Box::new(reedline::Vi::new(
			reedline::default_vi_insert_keybindings(),
			reedline::default_vi_normal_keybindings(),
		)))
		.with_highlighter(Box::new(reedline::ExampleHighlighter::new(commands)))
		.with_mouse_click(reedline::MouseClickMode::EnabledWithOsc133)
		.use_bracketed_paste(true);

	let mut session = String::new();
	let prompt = reedline::DefaultPrompt::new(
		reedline::DefaultPromptSegment::Basic("oi".to_string()),
		reedline::DefaultPromptSegment::Empty,
	);

	// TODO: add version and whatever else REPLs usually have in the greeting
	eprintln!("Oi! Type :help for help.");

	loop {
		match rl.read_line(&prompt) {
			Ok(reedline::Signal::Success(line)) => {
				match line.trim() {
					"" => continue,
					":help" | ":h" => {
						// TODO: print version too
						indoc::eprintdoc! {"
							The Oi REPL.

							Runs code you input as if it were running a script.
							The context persists, but it's just by concatenating all your input together,
							so if you run into any issues `:clear` it away.

							Commands:
								:h, :help -> help
								:q, :quit -> quit
								:x, :exit -> quit
								:c, :clear -> clear session context
						"};
						continue;
					}
					":quit" | ":q" | ":exit" | ":x" => {
						eprintln!("goodbye");
						break;
					}
					":clear" | ":c" => {
						eprintln!("session cleared");
						session.clear();
						continue;
					}
					_ => {}
				}

				let candidate = format!("{session}{line}\n");
				if run_source("<repl>", &candidate, std::path::Path::new("."), false).is_ok() {
					session = candidate;
				}
			}
			Ok(reedline::Signal::CtrlC) => continue,
			Ok(reedline::Signal::CtrlD) => break,
			Ok(_) => {}
			Err(e) => {
				eprintln!("oi: {e}");
				break;
			}
		}
	}

	Ok(())
}
