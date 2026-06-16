use chumsky::span::SimpleSpan;

// A value paired with the span it came from.
pub type Span = SimpleSpan;
pub type Spanned<T> = (T, Span);

#[allow(dead_code)]
#[derive(Debug)]
pub enum Expr {
	Bool(bool),
	Int(i32),
	Float(f64),
	String(String),
	Ident(String),

	Assign {
		mutable: bool,
		name: String,
		value: Box<Spanned<Expr>>,
	},

	Fn {
		name: String,
		body: Vec<Spanned<Expr>>,
	},

	Call(String),

	// unary operators
	Negative(Box<Spanned<Expr>>),

	// binary operators
	Add(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
	Sub(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
	Mul(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
	Div(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
}
