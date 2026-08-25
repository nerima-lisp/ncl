use ncl_syntax::{Form, Span};
use std::collections::HashSet;

#[derive(Default)]
pub(crate) struct CompileState {
    pub(crate) functions: Vec<FunctionCode>,
    pub(crate) local_function_scopes: Vec<HashSet<String>>,
    pub(crate) used_names: HashSet<String>,
    pub(crate) temporary_counter: usize,
}

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

pub type FunctionId = usize;

#[derive(Clone, Debug, PartialEq)]
pub struct OptionalParameter {
    pub name: String,
    pub name_escaped: bool,
    pub default_function: FunctionId,
    pub supplied_p: Option<String>,
    pub supplied_p_escaped: Option<bool>,
}
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
#[derive(Clone, Debug, PartialEq)]
pub struct AuxiliaryParameter {
    pub name: String,
    pub name_escaped: bool,
    pub default_function: FunctionId,
}
#[derive(Clone, Debug, PartialEq)]
pub struct HandlerCaseClause {
    pub condition: String,
    pub variable: Option<String>,
    pub function: FunctionId,
}
#[derive(Clone, Debug, PartialEq)]
pub struct HandlerBindClause {
    pub condition: String,
    pub function: FunctionId,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RestartBindClause {
    pub name: String,
    pub function: FunctionId,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RestartCaseClause {
    pub name: String,
    pub function: FunctionId,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DestructurePattern {
    Name(String),
    List(Vec<Self>),
    Dotted { items: Vec<Self>, tail: Box<Self> },
}
#[derive(Clone, Debug, PartialEq)]
pub struct DestructureOptionalParameter {
    pub pattern: DestructurePattern,
    pub default_function: FunctionId,
    pub supplied_p: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DestructureKeywordParameter {
    pub keyword_name: String,
    pub pattern: DestructurePattern,
    pub default_function: FunctionId,
    pub supplied_p: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DestructureAuxiliaryParameter {
    pub name: String,
    pub default_function: FunctionId,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DestructureLambdaList {
    pub whole: Option<String>,
    pub required: Vec<DestructurePattern>,
    pub optional: Vec<DestructureOptionalParameter>,
    pub keywords: Vec<DestructureKeywordParameter>,
    pub has_keyword_section: bool,
    pub allow_other_keys: bool,
    pub rest: Option<String>,
    pub auxiliary: Vec<DestructureAuxiliaryParameter>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum DestructureSpec {
    Pattern(DestructurePattern),
    LambdaList(DestructureLambdaList),
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum DestructureLambdaListSection {
    Required,
    Optional,
    Rest,
    Keyword,
    Auxiliary,
}

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
    DefineSpecial {
        name: String,
        force: bool,
    },
    DefineSpecialExact {
        name: String,
        force: bool,
    },
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
#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub functions: Vec<FunctionCode>,
    pub entry: FunctionId,
}
