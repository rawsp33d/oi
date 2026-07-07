use super::*;

impl<'a> Translator<'a> {
	// The named types in scope, bundled for resolving type annotations.
	pub(super) fn types(&self) -> TypeCtx<'a> {
		TypeCtx {
			structs: self.structs,
			enums: self.enums,
			aliases: self.aliases,
		}
	}

	// Look up the binding that a mutation targets.
	pub(super) fn mutable_local(&self, name: &str, span: Range<usize>, op: Mutation) -> Result<Local, Diagnostic> {
		// how the mutation reads in errors
		// (verb, verb when immutable, noun for the `mut` hint, suggest `:=`?)
		let (verb, immutable_verb, allow, suggest_declare) = match op {
			Mutation::Assign => ("assign to", "assign to", "assignment", true),
			Mutation::IndexAssign => ("assign to", "assign to element of", "assignment", true),
			Mutation::Append => ("append to", "append to", "append", false),
			Mutation::FieldAssign => ("assign field of", "assign field of", "field assignment", false),
		};
		let local = self.vars.get(name).cloned().ok_or_else(|| {
			let d = Diagnostic::new(format!("cannot {verb} undefined variable `{name}`"), span.clone())
				.with_label("not found in scope");
			if suggest_declare {
				d.with_note(format!("declare it first with `{name} := ...`"))
			} else {
				d
			}
		})?;
		if !local.mutable {
			return Err(
				Diagnostic::new(format!("cannot {immutable_verb} immutable `{name}`"), span)
					.with_label("declared without `mut`")
					.with_note(format!("use `mut {name} := ...` to allow {allow}")),
			);
		}
		Ok(local)
	}
}
