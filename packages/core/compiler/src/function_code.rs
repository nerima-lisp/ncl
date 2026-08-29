//! A compiled function's bytecode plus the compiled-program container.

use crate::{AuxiliaryParameter, FunctionId, Instruction, KeywordParameter, OptionalParameter};

/// The bytecode and metadata for one callable function.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionCode {
    /// Optional function name.
    pub name: Option<String>,
    /// Required parameter names.
    pub parameters: Vec<String>,
    /// Escaping flags for required parameters.
    pub required_escaped: Vec<bool>,
    /// Optional parameters.
    pub optional: Vec<OptionalParameter>,
    /// Keyword parameters.
    pub keywords: Vec<KeywordParameter>,
    /// Whether a keyword section was present.
    pub has_keyword_section: bool,
    /// Whether unknown keywords are accepted.
    pub allow_other_keys: bool,
    /// Optional rest parameter.
    pub rest: Option<String>,
    /// Whether the rest name was escaped.
    pub rest_escaped: bool,
    /// Auxiliary parameters.
    pub auxiliary: Vec<AuxiliaryParameter>,
    /// Stack bytecode instructions.
    pub instructions: Vec<Instruction>,
}

/// A compiled entry function and its nested function bodies.
#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    /// All function bodies, including nested functions.
    pub functions: Vec<FunctionCode>,
    /// Index of the entry function in [`Self::functions`].
    pub entry: FunctionId,
}
