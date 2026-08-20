use std::error::Error;
use std::fmt;

use ncl_syntax::{Form, Span};

/// A literal value embedded directly in bytecode.
#[derive(Clone, Debug, PartialEq)]
pub enum Constant {
    Nil,
    Boolean(bool),
    Integer(i64),
    Rational { numerator: i64, denominator: i64 },
    Float(f64),
    String(String),
    Character(char),
    Symbol(String),
    SymbolExact(String),
    Keyword(String),
    KeywordExact(String),
}

/// An index into [`Program::functions`].
pub type FunctionId = usize;

/// Metadata for one compiled `&OPTIONAL` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionalParameter {
    pub name: String,
    pub name_escaped: bool,
    pub default_function: FunctionId,
    pub supplied_p: Option<String>,
    pub supplied_p_escaped: Option<bool>,
}

/// Metadata for one compiled `&KEY` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct KeywordParameter {
    pub keyword_name: String,
    pub keyword_name_escaped: bool,
    pub name: String,
    pub name_escaped: bool,
    pub default_function: FunctionId,
    pub supplied_p: Option<String>,
    pub supplied_p_escaped: Option<bool>,
}

/// Metadata for one compiled `&AUX` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct AuxiliaryParameter {
    pub name: String,
    pub name_escaped: bool,
    pub default_function: FunctionId,
}

/// One compiled `HANDLER-CASE` clause.
#[derive(Clone, Debug, PartialEq)]
pub struct HandlerCaseClause {
    pub condition: String,
    pub variable: Option<String>,
    pub function: FunctionId,
}

/// One compiled `HANDLER-BIND` clause.
#[derive(Clone, Debug, PartialEq)]
pub struct HandlerBindClause {
    pub condition: String,
    pub function: FunctionId,
}

/// One compiled `RESTART-BIND` clause.
#[derive(Clone, Debug, PartialEq)]
pub struct RestartBindClause {
    pub name: String,
    pub function: FunctionId,
}

/// One compiled `RESTART-CASE` clause.
#[derive(Clone, Debug, PartialEq)]
pub struct RestartCaseClause {
    pub name: String,
    pub function: FunctionId,
}

/// A recursive pattern used by the `DESTRUCTURING-BIND` bytecode operation.
#[derive(Clone, Debug, PartialEq)]
pub enum DestructurePattern {
    Name(String),
    List(Vec<Self>),
    LambdaList(DestructureLambdaList),
    Dotted { items: Vec<Self>, tail: Box<Self> },
}

/// One compiled `DESTRUCTURING-BIND` `&OPTIONAL` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct DestructureOptionalParameter {
    pub pattern: DestructurePattern,
    pub default_function: FunctionId,
    pub supplied_p: Option<String>,
}

/// One compiled `DESTRUCTURING-BIND` `&KEY` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct DestructureKeywordParameter {
    pub keyword_name: String,
    pub pattern: DestructurePattern,
    pub default_function: FunctionId,
    pub supplied_p: Option<String>,
}

/// One compiled `DESTRUCTURING-BIND` `&AUX` parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct DestructureAuxiliaryParameter {
    pub name: String,
    pub default_function: FunctionId,
}

/// A compiled destructuring lambda list.
#[derive(Clone, Debug, PartialEq)]
pub struct DestructureLambdaList {
    pub whole: Option<String>,
    pub environment: Option<String>,
    pub required: Vec<DestructurePattern>,
    pub optional: Vec<DestructureOptionalParameter>,
    pub keywords: Vec<DestructureKeywordParameter>,
    pub has_keyword_section: bool,
    pub allow_other_keys: bool,
    pub rest: Option<String>,
    pub auxiliary: Vec<DestructureAuxiliaryParameter>,
}

/// The two forms accepted by the `DESTRUCTURING-BIND` bytecode operation.
#[derive(Clone, Debug, PartialEq)]
pub enum DestructureSpec {
    Pattern(DestructurePattern),
    LambdaList(DestructureLambdaList),
}

/// One stack-bytecode operation.
///
/// `Constant`, `Quote`, `QuasiQuote`, `Load`, `FunctionLoad`, `IsBound`, and
/// `MakeClosure` push one value. `Define`, `Set`, and `Setf` install the
/// primary value at the top of the stack and leave that value on the stack.
/// `DefineFunction` consumes a closure and stores it in the lexical function
/// namespace. `DefineValues` preserves a multiple-value carrier. `Psetq`
/// consumes all RHS values and leaves `NIL`; `MultipleValueSetq` consumes one
/// carrier and leaves its primary value. `JumpIfFalse` consumes its condition.
/// Scope operations do not alter the value stack, and call-like instructions
/// replace their callee and arguments with a result. `Values` creates one
/// carrier stack entry, `MultipleValueList` converts one carrier to a list,
/// while `BindValues` and `Destructure` consume one carrier without pushing a
/// result.
#[derive(Clone, Debug, PartialEq)]
pub enum Instruction {
    Constant(Constant),
    Quote(Form),
    QuasiQuote(Form),
    Load(String),
    LoadExact(String),
    FunctionLoad(String),
    FunctionLoadExact(String),
    IsBound(String),
    IsBoundExact(String),
    Define(String),
    DefineExact(String),
    DefineFunction(String),
    DefineFunctionExact(String),
    DefineFunctionDocumentation {
        name: String,
        exact: bool,
        documentation: String,
    },
    DefineVariableDocumentation {
        name: String,
        exact: bool,
        documentation: String,
    },
    DefineSpecial {
        name: String,
        force: bool,
    },
    DefineSpecialExact {
        name: String,
        force: bool,
    },
    CheckConstant(String),
    CheckConstantExact(String),
    DefineConstant(String),
    DefineConstantExact(String),
    DefineValues(String),
    DefineValuesExact(String),
    Set(String),
    SetExact(String),
    Setf(Form),
    MapIntoSetf(Form),
    Psetq(Vec<String>),
    PsetqExact(Vec<(String, bool)>),
    MultipleValueSetq(Vec<String>),
    MultipleValueSetqExact(Vec<(String, bool)>),
    EnterScope,
    ExitScope,
    Pop,
    Dup,
    Primary,
    Values(usize),
    NthValue(Span),
    LoadTimeValue,
    MultipleValueList,
    BindValues(Vec<String>),
    BindValuesExact(Vec<(String, bool)>),
    Destructure(DestructureSpec),
    JumpIfFalse(usize),
    Jump(usize),
    MakeClosure(FunctionId),
    IgnoreErrors(FunctionId),
    HandlerCase {
        protected: FunctionId,
        clauses: Vec<HandlerCaseClause>,
    },
    HandlerBind {
        body: FunctionId,
        handlers: Vec<HandlerBindClause>,
    },
    RestartBind {
        body: FunctionId,
        bindings: Vec<RestartBindClause>,
    },
    Catch {
        tag: FunctionId,
        body: FunctionId,
    },
    WithSimpleRestart {
        name: String,
        body: FunctionId,
    },
    WithConditionRestarts {
        condition: FunctionId,
        restarts: FunctionId,
        body: FunctionId,
    },
    RestartCase {
        protected: FunctionId,
        clauses: Vec<RestartCaseClause>,
    },
    Progv {
        symbols: FunctionId,
        values: FunctionId,
        body: FunctionId,
    },
    Throw,
    Block {
        function: FunctionId,
        name: String,
    },
    TagBody {
        function: FunctionId,
        tags: Vec<(String, usize)>,
    },
    UnwindProtect {
        protected: FunctionId,
        cleanup: FunctionId,
    },
    ReturnFrom {
        name: String,
    },
    Go {
        tag: String,
    },
    Eval(Span),
    Call(usize),
    Apply(usize),
    MapCar(usize),
    MultipleValueCall(usize),
    Return,
}

/// The bytecode and metadata for one callable function.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionCode {
    pub name: Option<String>,
    pub parameters: Vec<String>,
    pub required_escaped: Vec<bool>,
    pub optional: Vec<OptionalParameter>,
    pub keywords: Vec<KeywordParameter>,
    pub has_keyword_section: bool,
    pub allow_other_keys: bool,
    pub rest: Option<String>,
    pub rest_escaped: bool,
    pub auxiliary: Vec<AuxiliaryParameter>,
    pub instructions: Vec<Instruction>,
}

/// A compiled entry function and its nested function bodies.
#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub functions: Vec<FunctionCode>,
    pub entry: FunctionId,
}

impl Program {
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    pub fn instruction_count(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.instructions.len())
            .sum()
    }
}

/// The category of a compile-time error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileErrorKind {
    Arity {
        operator: String,
        expected: String,
        actual: usize,
    },
    ExpectedList {
        context: String,
    },
    ExpectedSymbol {
        context: String,
    },
    InvalidForm {
        message: String,
    },
    UnsupportedForm {
        message: String,
    },
    Internal {
        message: String,
    },
}

/// A typed compiler error tied to the source span that caused it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub span: Span,
}

impl CompileError {
    pub fn new(kind: CompileErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

impl fmt::Display for CompileErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arity {
                operator,
                expected,
                actual,
            } => write!(
                formatter,
                "{operator} expected {expected} arguments, received {actual}"
            ),
            Self::ExpectedList { context } => write!(formatter, "{context} must be a list"),
            Self::ExpectedSymbol { context } => {
                write!(formatter, "{context} must be a symbol")
            }
            Self::InvalidForm { message }
            | Self::UnsupportedForm { message }
            | Self::Internal { message } => formatter.write_str(message),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at byte {}..{}",
            self.kind, self.span.start, self.span.end
        )
    }
}

impl Error for CompileError {}
