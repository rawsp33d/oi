//! Multi-file module loading.
//! Read imported dirs, parse and validate each file, and qualify definition names as `module::name`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use chumsky::{input::Stream, prelude::*};

use crate::Reported;
use crate::ast::{Expr, Span, Spanned, TypeExpr, UseItem};
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
	pub visible: HashMap<String, Visible>,
}

// A visible module.
pub struct Visible {
	pub module: String,
	pub only: Option<HashMap<String, String>>,
}

// A whole program, with its source files and modules and pubs, oh my.
pub struct Program {
	pub map: SourceMap,
	pub modules: Vec<Module>,
	pub publics: HashSet<String>,
	pub reexports: HashMap<String, String>,
	pub consts: HashMap<String, Spanned<Expr>>,
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

// Ensure const values are literals.
fn is_literal(e: &Expr) -> bool {
	match e {
		Expr::Bool(_) | Expr::Int(_) | Expr::Float(_) | Expr::String(_) | Expr::Atom(_) => true,
		Expr::Negative(inner) => is_literal(&inner.0),
		_ => false,
	}
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
	reexports: HashMap<String, String>,
	consts: HashMap<String, Spanned<Expr>>,
	// import stack
	loading: Vec<String>,
	selected: Vec<(String, String, Span)>,
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
			if matches!(item.0, Expr::Module(_)) {
				return Err(err("`module` must come first", item.1, "move it to the top"));
			}
			// peel off `pub` wrapper
			let public = matches!(item.0, Expr::Pub(_));
			let mut item = match item {
				(Expr::Pub(inner), _) => *inner,
				item => item,
			};
			if let Expr::Use { name, path, group } = &item.0 {
				let (module, _) = &path[0];
				if path.len() > 2 || (path.len() == 2 && group.is_some()) {
					return Err(err(
						"nested module paths aren't supported yet",
						item.1,
						"flatten the path",
					));
				}
				// a `.item` import tail acts as a one-item group
				let items: Vec<UseItem> = match (path.get(1), group) {
					(Some(it), _) => vec![UseItem {
						local: name.clone().unwrap_or_else(|| it.clone()),
						rename_of: Some(it.clone()),
					}],
					(None, Some(items)) => items.clone(),
					(None, None) => vec![],
				};
				// ensure every imported item is public in its module
				for it in &items {
					let (remote, span) = it.remote();
					self.selected.push((module.clone(), remote.clone(), *span));
				}
				let narrows = name.is_some() && group.is_some();
				if public && (items.is_empty() || narrows) {
					return Err(err(
						"only item imports can be re-exported yet",
						item.1,
						"import it privately instead",
					));
				}
				if narrows || items.is_empty() {
					// bind the module itself, or narrowed to its specified items
					let local = name.as_ref().map_or(module, |(n, _)| n).clone();
					let only =
						narrows.then(|| items.iter().map(|it| (it.local.0.clone(), it.remote().0.clone())).collect());
					let vis = Visible {
						module: module.clone(),
						only,
					};
					// handle re-importing
					if let Some(prev) = m.scope.visible.insert(local.clone(), vis)
						&& (narrows || prev.only.is_some() || prev.module != *module)
					{
						return Err(err(
							format!("`{local}` already names module `{}`", prev.module),
							item.1,
							"conflicting import",
						));
					}
				} else {
					// bind the items
					for it in &items {
						let (local, span) = &it.local;
						let target = format!("{module}::{}", it.remote().0);
						if public {
							self.reexports.insert(format!("{}::{local}", m.name), target.clone());
						}
						if m.scope.env.insert(local.clone(), target).is_some() {
							let msg = format!("`{local}` is already defined in module `{}`", m.name);
							return Err(err(msg, *span, "conflicting import"));
						}
					}
				}
				imports.push((module.clone(), item.1));
				continue;
			}
			let span = item.1;
			match &mut item.0 {
				Expr::Fn { name, .. }
				| Expr::StructDef { name, .. }
				| Expr::EnumDef { name, .. }
				| Expr::TypeAlias { name, .. } => self.define(m, name, !main, public, span)?,
				Expr::TraitDef { name, .. } => self.define(m, name, false, public, span)?,
				Expr::Bind {
					mutable,
					name,
					typ,
					value,
				} if !main => {
					let bad = match (*mutable, typ.is_some(), value.as_deref()) {
						(true, ..) => Some(("a module-level binding must be a const", "use `::`")),
						(_, true, _) => {
							Some(("type annotations on consts aren't supported yet", "drop the annotation"))
						}
						(_, _, Some(v)) if is_literal(&v.0) => {
							self.define(m, name, true, public, span)?;
							self.consts.insert(name.clone(), v.clone());
							continue;
						}
						(_, _, Some(v)) if TypeExpr::from_expr(&v.0).is_some() => {
							self.define(m, name, true, public, span)?;
							None
						}
						_ => Some(("only literal consts are allowed in a module", "not a literal")),
					};
					if let Some((msg, label)) = bad {
						return Err(err(msg, span, label));
					}
				}
				Expr::Claim { typ, .. } if !main => *typ = format!("{}::{typ}", m.name),
				Expr::Claim { .. } | Expr::Doc(_) => {}
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

	// Collapse `pub use` chains to their final targets, then point every binding at them.
	fn resolve_reexports(&mut self) -> HashMap<String, String> {
		let resolved: HashMap<_, _> = self
			.reexports
			.keys()
			.map(|alias| {
				let mut target = &self.reexports[alias];
				while let Some(next) = self.reexports.get(target) {
					target = next;
				}
				(alias.clone(), target.clone())
			})
			.collect();
		for m in &mut self.modules {
			for target in m.scope.env.values_mut() {
				if let Some(t) = resolved.get(target) {
					*target = t.clone();
				}
			}
		}
		resolved
	}

	// Ensure selected names are public within their module.
	fn check_selected(&self) -> Result<(), Reported> {
		for (module, name, span) in &self.selected {
			let m = self.modules.iter().find(|m| &m.name == module).unwrap();
			let is_def = |q: &String| {
				self.consts.contains_key(q)
					|| self.modules.iter().any(|m| {
						m.items.iter().any(|i| {
							matches!(&i.0,
								Expr::Fn { name, .. }
								| Expr::StructDef { name, .. }
								| Expr::EnumDef { name, .. }
								| Expr::TypeAlias { name, .. }
								| Expr::Bind { name, .. } if name == q)
						})
					})
			};
			let (msg, label) = match m.scope.env.get(name) {
				None => (format!("module `{module}` has no `{name}`"), "no such name"),
				Some(q) if !is_def(q) => (
					format!("`{name}` cannot be imported"),
					"only fns and types can be imported for now",
				),
				Some(q) if !self.publics.contains(q) => {
					(format!("`{name}` is private to module `{module}`"), "not public")
				}
				_ => continue,
			};
			return Err(self.report(err(msg, *span, label)));
		}
		Ok(())
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
		reexports: HashMap::new(),
		consts: HashMap::new(),
		loading: vec![],
		selected: vec![],
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
	let reexports = loader.resolve_reexports();
	loader.check_selected()?;
	Ok(Program {
		map: loader.map,
		modules: loader.modules,
		publics: loader.publics,
		reexports,
		consts: loader.consts,
	})
}
