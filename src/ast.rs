use std::fmt;

use chumsky::span::SimpleSpan;

pub type Span = SimpleSpan;
pub type Spanned<T> = (T, Span);

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Expr {
	// literals
	Bool(bool),
	Int(i64),
	Float(f64),
	String(String),
	Atom(String),
	Ident(String),
	Dollar,
	None,

	// `[mods] name [type] := value`: declares a new binding
	Bind {
		mutable: bool,
		name: String,
		typ: Option<Spanned<TypeExpr>>,
		value: Option<Box<Spanned<Expr>>>,
	},

	// `T{}`
	TypeInit(Spanned<TypeExpr>, Vec<Spanned<Expr>>),

	// `?T(value)`, `?T(none)`
	OptionInit {
		inner: Spanned<TypeExpr>,
		arg: Box<Spanned<Expr>>,
	},

	// `!T(value)`, `!T(error)`
	ResultInit {
		inner: Spanned<TypeExpr>,
		arg: Box<Spanned<Expr>>,
	},

	// `name = value`: assigns to an existing mutable binding
	Assign {
		name: String,
		value: Box<Spanned<Expr>>,
	},

	// `(a, mut b) := value`, `(a, b) = value`
	Destructure {
		names: Vec<(bool, String)>,
		value: Box<Spanned<Expr>>,
		bind: bool,
	},

	Fn {
		name: String,
		type_params: Vec<TypeParam>,
		params: Vec<Param>,
		params_tuple: bool,
		ret: Option<Spanned<TypeExpr>>,
		body: Vec<Spanned<Expr>>,
	},

	// `fn [captures]? (params)? ret? { body }`
	AnonFn {
		captures: Option<Vec<Capture>>,
		params: Vec<Param>,
		params_tuple: bool,
		ret: Option<Spanned<TypeExpr>>,
		body: Vec<Spanned<Expr>>,
	},

	Call {
		name: String,
		type_args: Vec<Spanned<TypeExpr>>,
		args: Vec<Spanned<Expr>>,
	},

	MutArg(Box<Spanned<Expr>>),

	MethodCall {
		recv: Box<Spanned<Expr>>,
		method: String,
		args: Vec<Spanned<Expr>>,
	},

	Return(Option<Box<Spanned<Expr>>>),

	// control flow
	If {
		cond: Box<Spanned<Expr>>,
		then: Vec<Spanned<Expr>>,
		els: Option<Vec<Spanned<Expr>>>,
	},

	Loop {
		cond: Option<Box<Spanned<Expr>>>,
		body: Vec<Spanned<Expr>>,
	},

	// `loop <pat> in <iter> {}`
	For {
		pat: Pattern,
		iter: Box<Spanned<Expr>>,
		body: Vec<Spanned<Expr>>,
	},

	Break,
	Continue,

	// structures

	// tuples
	Tuple(Vec<(Option<String>, Spanned<Expr>)>),
	Field {
		tuple: Box<Spanned<Expr>>,
		field: String,
	},

	// arrays
	Array(Vec<Spanned<Expr>>),
	// `collection[index]`
	// TODO: handle negative indices
	Index {
		collection: Box<Spanned<Expr>>,
		index: Box<Spanned<Expr>>,
	},
	// `collection[start?..end?]`
	Slice {
		collection: Box<Spanned<Expr>>,
		start: Option<Box<Spanned<Expr>>>,
		end: Option<Box<Spanned<Expr>>>,
	},
	// `name[index] = value`
	IndexAssign {
		name: String,
		index: Box<Spanned<Expr>>,
		value: Box<Spanned<Expr>>,
	},
	// `name << value`
	Append {
		name: String,
		value: Box<Spanned<Expr>>,
	},
	// `name.delete[key]`
	MapDelete {
		name: String,
		key: Box<Spanned<Expr>>,
	},

	// `match subject { pattern, ... { body } ... else { body } }`
	Match {
		subject: Box<Spanned<Expr>>,
		arms: Vec<MatchArm>,
		else_body: Option<Vec<Spanned<Expr>>>,
	},

	// `value |> step`
	Pipe {
		value: Box<Spanned<Expr>>,
		step: Box<Spanned<Expr>>,
	},

	// `value or { body }`
	OrElse {
		value: Box<Spanned<Expr>>,
		body: Vec<Spanned<Expr>>,
	},

	// `value?`, unwraps `?T`/`!T`
	Propagate(Box<Spanned<Expr>>),

	// structs
	// `Name :: struct {}`
	StructDef {
		name: String,
		type_params: Vec<TypeParam>,
		fields: Vec<Param>,
		fills: Vec<Spanned<Expr>>,
	},
	// `Name {}`
	StructLit {
		name: String,
		fields: Vec<(Option<String>, Spanned<Expr>)>,
	},
	// `&Name {}`
	Ref(Box<Spanned<Expr>>),
	// `{ k = v }`
	Record(Vec<(Spanned<Expr>, Spanned<Expr>)>),
	// `name.field = value`
	FieldAssign {
		name: String,
		field: String,
		value: Box<Spanned<Expr>>,
	},

	// `Type :{ fills }`
	Claim {
		typ: String,
		type_params: Vec<TypeParam>,
		traits: Vec<String>,
		fills: Vec<Spanned<Expr>>,
	},

	// `trait Name {}`
	TraitDef {
		name: String,
		supers: Vec<String>,
		fields: Vec<Param>,
		methods: Vec<Spanned<Expr>>,
	},

	// `type Name = TypeExpr`
	TypeAlias {
		name: String,
		typ: TypeExpr,
	},

	// `start..end`, `start..`, `..end`
	Range {
		start: Option<Box<Spanned<Expr>>>,
		end: Option<Box<Spanned<Expr>>>,
	},

	// `Name : backing? : enum {}`
	EnumDef {
		name: String,
		backing: Option<Spanned<TypeExpr>>,
		type_params: Vec<TypeParam>,
		variants: Vec<EnumVariant>,
		fills: Vec<Spanned<Expr>>,
	},
	// `.variant`, `.variant(args)`
	EnumShorthand {
		variant: String,
		args: Vec<Spanned<Expr>>,
	},

	// modules

	// `module name`
	Module(String),
	// `use path`, `name :: use path.{ local :: remote }`
	Use {
		name: Option<Spanned<String>>,
		path: Vec<Spanned<String>>,
		group: Option<Vec<UseItem>>,
	},
	// `pub expr`
	Pub(Box<Spanned<Expr>>),

	// operators

	// `T is not? Trait`
	Is {
		subject: Box<Spanned<Expr>>,
		trait_name: String,
		negated: bool,
	},

	// unary
	Negative(Box<Spanned<Expr>>),

	// arithmetic, comparison, logical, membership
	Binary(BinOp, Box<Spanned<Expr>>, Box<Spanned<Expr>>),
	Not(Box<Spanned<Expr>>),

	// meta
	Doc(Vec<String>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
	Add,
	Sub,
	Mul,
	Div,
	Mod,
	Eq,
	Ne,
	Lt,
	Gt,
	Le,
	Ge,
	And,
	Or,
	In,
}

impl fmt::Display for BinOp {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(match self {
			BinOp::Add => "+",
			BinOp::Sub => "-",
			BinOp::Mul => "*",
			BinOp::Div => "/",
			BinOp::Mod => "%",
			BinOp::Eq => "==",
			BinOp::Ne => "!=",
			BinOp::Lt => "<",
			BinOp::Gt => ">",
			BinOp::Le => "<=",
			BinOp::Ge => ">=",
			BinOp::And => "&&",
			BinOp::Or => "||",
			BinOp::In => "in",
		})
	}
}

// Type annotation.
#[derive(Debug, Clone)]
pub enum TypeExpr {
	Name(String),
	Tuple(Vec<TypeExpr>),
	Array(Box<TypeExpr>),
	FixedArray(Box<TypeExpr>, usize),
	Fn(Vec<TypeExpr>, Vec<bool>, Box<TypeExpr>),
	Option(Box<TypeExpr>),
	Result(Box<TypeExpr>, Option<Box<TypeExpr>>),
	AtomSum(Vec<String>),
	Sum(Vec<TypeExpr>),
	TupleStruct(String, Vec<(Option<String>, TypeExpr)>),
	Map(Box<TypeExpr>, Box<TypeExpr>),
	Generic(String, Vec<TypeExpr>),
	Ref(Box<TypeExpr>),
}

impl TypeExpr {
	// The type an expression could be naming.
	pub fn from_expr(e: &Expr) -> Option<TypeExpr> {
		match e {
			Expr::Ident(n) => Some(TypeExpr::Name(n.clone())),
			Expr::Tuple(fields) if !fields.is_empty() && fields.iter().all(|(n, _)| n.is_none()) => fields
				.iter()
				.map(|(_, v)| TypeExpr::from_expr(&v.0))
				.collect::<Option<_>>()
				.map(TypeExpr::Tuple),
			Expr::Index { collection, index } => match &collection.0 {
				Expr::Ident(n) => Some(TypeExpr::Generic(n.clone(), vec![TypeExpr::from_expr(&index.0)?])),
				_ => None,
			},
			_ => None,
		}
	}
}

#[derive(Debug, Clone)]
// One arm of a `match` expression.
// `patterns` are compared to the subject (OR'd together).
// `binding @` names the subject value for the arm body.
// `body` runs when any pattern matches.
pub struct MatchArm {
	pub binding: Option<String>,
	pub patterns: Vec<Spanned<Expr>>,
	pub body: Vec<Spanned<Expr>>,
}

#[derive(Debug, Clone)]
pub struct UseItem {
	pub local: Spanned<String>,
	pub rename_of: Option<Spanned<String>>,
}

impl UseItem {
	pub fn remote(&self) -> &Spanned<String> {
		self.rename_of.as_ref().unwrap_or(&self.local)
	}
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
	pub name: String,
	pub disc: Option<i64>,
	pub raw: Option<String>,
	pub payload: Vec<Spanned<TypeExpr>>,
	pub names: Vec<String>,
}

// A `loop` binding pattern (name or destruction).
#[derive(Debug, Clone)]
pub enum Pattern {
	Name(String),
	Tuple(Vec<String>),
}

// Capture list entry of an anon fn.
#[derive(Debug, Clone)]
pub enum Capture {
	ReadOnly(String),
	Mut(String),
	Move(String),
}

// Generic type parameter.
#[derive(Debug, Clone)]
pub struct TypeParam {
	pub name: String,
	pub bound: Option<String>,
}

// A function parameter or struct field declaration.
#[derive(Debug, Clone)]
pub struct Param {
	pub name: String,
	pub typ: TypeExpr,
	pub span: Span,
	pub default: Option<Spanned<Expr>>,
	pub mutable: bool,
}
