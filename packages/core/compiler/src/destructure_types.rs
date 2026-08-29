//! Compiled representations of `DESTRUCTURING-BIND` patterns and lambda lists.

use crate::FunctionId;

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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DestructurePattern {
    /// A single symbol binding.
    Name(String),
    /// A nested proper list pattern.
    List(Vec<Self>),
    /// A dotted list pattern.
    Dotted {
        /// Proper-list prefix patterns.
        items: Vec<Self>,
        /// Pattern for the dotted tail.
        tail: Box<Self>,
    },
}

/// One compiled `DESTRUCTURING-BIND` `&OPTIONAL` parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestructureOptionalParameter {
    /// Binding pattern.
    pub pattern: DestructurePattern,
    /// Function containing the init-form.
    pub default_function: FunctionId,
    /// Optional supplied-p binding name.
    pub supplied_p: Option<String>,
}

/// One compiled `DESTRUCTURING-BIND` `&KEY` parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestructureKeywordParameter {
    /// External keyword name.
    pub keyword_name: String,
    /// Binding pattern.
    pub pattern: DestructurePattern,
    /// Function containing the init-form.
    pub default_function: FunctionId,
    /// Optional supplied-p binding name.
    pub supplied_p: Option<String>,
}

/// One compiled `DESTRUCTURING-BIND` `&AUX` parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestructureAuxiliaryParameter {
    /// Auxiliary binding name.
    pub name: String,
    /// Function containing the init-form.
    pub default_function: FunctionId,
}

/// A compiled destructuring lambda list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestructureLambdaList {
    /// Optional `&WHOLE` binding name.
    pub whole: Option<String>,
    /// Required binding patterns.
    pub required: Vec<DestructurePattern>,
    /// Optional parameters.
    pub optional: Vec<DestructureOptionalParameter>,
    /// Keyword parameters.
    pub keywords: Vec<DestructureKeywordParameter>,
    /// Whether a `&KEY` section was present.
    pub has_keyword_section: bool,
    /// Whether unknown keywords are accepted.
    pub allow_other_keys: bool,
    /// Optional rest binding name.
    pub rest: Option<String>,
    /// Auxiliary parameters.
    pub auxiliary: Vec<DestructureAuxiliaryParameter>,
}

/// The two forms accepted by the `DESTRUCTURING-BIND` bytecode operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DestructureSpec {
    /// A single recursive pattern.
    Pattern(DestructurePattern),
    /// A complete destructuring lambda list.
    LambdaList(DestructureLambdaList),
}

/// The section of a destructuring lambda list currently being parsed.
///
/// Internal to [`crate::destructuring`]; not part of the crate's public API.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum DestructureLambdaListSection {
    Required,
    Optional,
    Rest,
    Keyword,
    Auxiliary,
}
