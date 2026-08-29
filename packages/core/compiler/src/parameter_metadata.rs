//! Metadata for compiled `&OPTIONAL`, `&KEY`, and `&AUX` parameters.

use crate::FunctionId;

/// Metadata for one compiled `&OPTIONAL` parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptionalParameter {
    /// Parameter binding name.
    pub name: String,
    /// Whether the name was escaped in source.
    pub name_escaped: bool,
    /// Function containing the init-form.
    pub default_function: FunctionId,
    /// Optional supplied-p binding name.
    pub supplied_p: Option<String>,
    /// Whether the supplied-p name was escaped.
    pub supplied_p_escaped: Option<bool>,
}

/// Metadata for one compiled `&KEY` parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeywordParameter {
    /// External keyword name.
    pub keyword_name: String,
    /// Whether the keyword name was escaped.
    pub keyword_name_escaped: bool,
    /// Local parameter binding name.
    pub name: String,
    /// Whether the local name was escaped.
    pub name_escaped: bool,
    /// Function containing the init-form.
    pub default_function: FunctionId,
    /// Optional supplied-p binding name.
    pub supplied_p: Option<String>,
    /// Whether the supplied-p name was escaped.
    pub supplied_p_escaped: Option<bool>,
}

/// Metadata for one compiled `&AUX` parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuxiliaryParameter {
    /// Auxiliary binding name.
    pub name: String,
    /// Whether the name was escaped in source.
    pub name_escaped: bool,
    /// Function containing the init-form.
    pub default_function: FunctionId,
}
