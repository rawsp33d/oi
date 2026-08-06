//! Multi-file module loading.
//! Read imported dirs, parse and validate each file, and qualify definition names as `module::name`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use chumsky::{input::Stream, prelude::*};

use crate::Reported;
use crate::ast::{Expr, Span, Spanned};
use crate::diagnostics::{Diagnostic, SourceMap};
use crate::lexer::lex_at;
use crate::parser::parser;

// A parsed module.
pub struct Module {
	pub name: String,
	pub items: Vec<Spanned<Expr>>,
	pub scope: Scope,
}

// A module's view of names.
#[derive(Default)]
pub struct Scope {
	pub env: HashMap<String, String>,
	pub visible: HashSet<String>,
}

// A whole program, with its source files and modules and pubs, oh my.
pub struct Program {
	pub map: SourceMap,
	pub modules: Vec<Module>,
	pub publics: HashSet<String>,
}

impl Program {
	// All items with their module's scope.
	pub fn items(&self) -> impl Iterator<Item = (&Scope, &Spanned<Expr>)> {
		self.modules.iter().flat_map(|m| m.items.iter().map(move |i| (&m.scope, i)))
	}
}

fn err(msg: impl Into<String>, span: Span, label: &str) -> Diagnostic {
	Diagnostic::new(msg.into(), span.into_range()).with_label(label)
}

// Lex and parse the file just pushed onto the map.
fn parse_file(map: &SourceMap, base: usize) -> Result<Vec<Spanned<Expr>>, Reported> {
	let src = map.last_src();
	let toks = lex_at(src, base);
	let eoi = (base + src.len()..base + src.len()).into();
	parser()
		.parse(Stream::from_iter(toks).map(eoi, |t| t))
		.into_result()
		.map_err(|errs| {
			for e in &errs {
				Diagnostic::from_rich(e).report_mapped(map);
			}
			Reported
		})
}

struct Loader<'a> {
	root: &'a Path,
	map: SourceMap,
	modules: Vec<Module>,
	publics: HashSet<String>,
	// import stack
	loading: Vec<String>,
}

impl Loader<'_> {
	fn report(&self, diag: Diagnostic) -> Reported {
		diag.report_mapped(&self.map);
		Reported
	}

	// Record a definition.
	fn define(
		&mut self,
		m: &mut Module,
		name: &mut String,
		qualify: bool,
		public: bool,
		span: Span,
	) -> Result<(), Diagnostic> {
		let bare = name.clone();
		if qualify {
			*name = format!("{}::{bare}", m.name);
		}
		if m.scope.env.insert(bare.clone(), name.clone()).is_some() {
			let msg = format!("`{bare}` is defined twice in module `{}`", m.name);
			return Err(err(msg, span, "duplicate definition"));
		}
		if public {
			self.publics.insert(name.clone());
		}
		Ok(())
	}

	// Validate a file and fold its items into the module, qualifying names as they land.
	fn add_file(
		&mut self,
		m: &mut Module,
		imports: &mut Vec<(String, Span)>,
		mut file: Vec<Spanned<Expr>>,
	) -> Result<(), Diagnostic> {
		let main = m.name == "main";
		// enforce V-like module declaration rules (for now, as a pretty sane starting point)
		match file.first() {
			Some((Expr::Module(name), span)) if main && name != "main" => {
				return Err(err("the entry file is module `main`", *span, "rename it to `main`"));
			}
			Some((Expr::Module(name), span)) if *name != m.name => {
				return Err(err(
					format!("this file must declare `module {}`", m.name),
					*span,
					"wrong module name",
				));
			}
			Some((Expr::Module(_), _)) => {
				file.remove(0);
			}
			Some((_, span)) if !main => {
				return Err(err(
					format!("this file must declare `module {}`", m.name),
					*span,
					"add it as the first line",
				));
			}
			_ => {}
		}
		for item in file {
			match &item.0 {
				Expr::Module(_) => return Err(err("`module` must come first", item.1, "move it to the top")),
				Expr::Import { module, alias, names } => {
					if alias.is_some() || !names.is_empty() {
						let what = if alias.is_some() {
							"import aliases"
						} else {
							"selective imports"
						};
						return Err(err(
							format!("{what} aren't supported yet"),
							item.1,
							"import the whole module",
						));
					}
					m.scope.visible.insert(module.clone());
					imports.push((module.clone(), item.1));
					continue;
				}
				_ => {}
			}
			// peel off `pub` wrapper
			let public = matches!(item.0, Expr::Pub(_));
			let mut item = match item {
				(Expr::Pub(inner), _) => *inner,
				item => item,
			};
			let span = item.1;
			match &mut item.0 {
				Expr::Fn { name, .. }
				| Expr::StructDef { name, .. }
				| Expr::EnumDef { name, .. }
				| Expr::TypeAlias { name, .. } => self.define(m, name, !main, public, span)?,
				Expr::TraitDef { name, .. } => self.define(m, name, false, public, span)?,
				Expr::Impl { typ, .. } if !main => *typ = format!("{}::{typ}", m.name),
				Expr::Impl { .. } | Expr::Doc(_) => {}
				_ if !main => {
					return Err(err(
						"top-level statements aren't allowed in a module",
						span,
						"only definitions and imports",
					));
				}
				_ => {}
			}
			m.items.push(item);
		}
		Ok(())
	}

	// Seal a module, then load its imports.
	fn seal(&mut self, module: Module, imports: Vec<(String, Span)>) -> Result<(), Reported> {
		self.loading.push(module.name.clone());
		self.modules.push(module);
		for (name, span) in imports {
			self.load_module(&name, span)?;
		}
		self.loading.pop();
		Ok(())
	}

	// Load a dir as one module.
	fn load_module(&mut self, name: &str, span: Span) -> Result<(), Reported> {
		if self.loading.iter().any(|m| m == name) {
			let msg = format!("import cycle: {} -> {name}", self.loading.join(" -> "));
			return Err(self.report(err(msg, span, "closes a cycle")));
		}
		if self.modules.iter().any(|m| m.name == name) {
			return Ok(());
		}
		let mut files: Vec<_> = fs::read_dir(self.root.join(name))
			.into_iter()
			.flatten()
			.flatten()
			.map(|e| e.path())
			.filter(|p| p.extension().is_some_and(|x| x == "oi"))
			.collect();
		files.sort();
		if files.is_empty() {
			return Err(self.report(err(format!("cannot find module `{name}`"), span, "no such module")));
		}
		let mut module = Module {
			name: name.to_string(),
			items: vec![],
			scope: Scope::default(),
		};
		let mut imports = vec![];
		for path in files {
			let src = fs::read_to_string(&path).unwrap_or_default();
			let base = self.map.push(path.display().to_string(), src);
			let items = parse_file(&self.map, base)?;
			self.add_file(&mut module, &mut imports, items).map_err(|d| self.report(d))?;
		}
		self.seal(module, imports)
	}
}

// Load the whole program starting from the entry source.
// `root` anchors module lookups.
pub fn load(entry_name: &str, entry_src: String, root: &Path) -> Result<Program, Reported> {
	let mut loader = Loader {
		root,
		map: SourceMap::default(),
		modules: vec![],
		publics: HashSet::new(),
		loading: vec![],
	};
	let base = loader.map.push(entry_name.to_string(), entry_src);
	let items = parse_file(&loader.map, base)?;
	let mut main = Module {
		name: "main".into(),
		items: vec![],
		scope: Scope::default(),
	};
	let mut imports = vec![];
	loader.add_file(&mut main, &mut imports, items).map_err(|d| loader.report(d))?;
	loader.seal(main, imports)?;
	Ok(Program {
		map: loader.map,
		modules: loader.modules,
		publics: loader.publics,
	})
}
