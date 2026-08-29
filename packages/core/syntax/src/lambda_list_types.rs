use std::error::Error;
use std::fmt;

use crate::{Form, Span};

/// The ordinary lambda-list shape shared by the compiler and evaluator.
#[derive(Clone, Debug, PartialEq)]
pub struct OrdinaryLambdaList {
    /// Required parameter names.
    pub required: Vec<String>,
    /// Whether each required name used escaping.
    pub required_escaped: Vec<bool>,
    /// Optional parameters.
    pub optional: Vec<LambdaListOptionalParameter>,
    /// Rest parameter name.
    pub rest: Option<String>,
    /// Whether the rest name used escaping.
    pub rest_escaped: bool,
    /// Keyword parameters.
    pub keywords: Vec<LambdaListKeywordParameter>,
    /// Whether an `&KEY` section was present.
    pub has_keyword_section: bool,
    /// Whether unknown keywords are accepted.
    pub allow_other_keys: bool,
    /// Auxiliary parameters.
    pub auxiliary: Vec<LambdaListAuxiliaryParameter>,
}

/// One `&OPTIONAL` parameter specification.
#[derive(Clone, Debug, PartialEq)]
pub struct LambdaListOptionalParameter {
    /// Parameter name.
    pub name: String,
    /// Whether the name used escaping.
    pub name_escaped: bool,
    /// Initialization form.
    pub init_form: Form,
    /// Whether an initialization form was explicitly supplied.
    pub init_form_supplied: bool,
    /// `supplied-p` variable name.
    pub supplied_p: Option<String>,
    /// Whether the `supplied-p` name used escaping.
    pub supplied_p_escaped: Option<bool>,
}

/// One `&KEY` parameter specification.
#[derive(Clone, Debug, PartialEq)]
pub struct LambdaListKeywordParameter {
    /// External keyword name.
    pub keyword_name: String,
    /// Whether the keyword name used escaping.
    pub keyword_name_escaped: bool,
    /// Local parameter name.
    pub name: String,
    /// Whether the local name used escaping.
    pub name_escaped: bool,
    /// Initialization form.
    pub init_form: Form,
    /// Whether an initialization form was explicitly supplied.
    pub init_form_supplied: bool,
    /// `supplied-p` variable name.
    pub supplied_p: Option<String>,
    /// Whether the `supplied-p` name used escaping.
    pub supplied_p_escaped: Option<bool>,
}

/// One `&AUX` parameter specification.
#[derive(Clone, Debug, PartialEq)]
pub struct LambdaListAuxiliaryParameter {
    /// Parameter name.
    pub name: String,
    /// Whether the name used escaping.
    pub name_escaped: bool,
    /// Initialization form.
    pub init_form: Form,
}

/// The category of an ordinary lambda-list syntax error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LambdaListErrorKind {
    /// The parameter form was not a proper list.
    ExpectedList,
    /// A symbol was required in the named context.
    ExpectedSymbol {
        /// Parameter-list context.
        context: &'static str,
    },
    /// The form violated lambda-list syntax.
    InvalidForm {
        /// Human-readable validation detail.
        message: String,
    },
}

/// A lambda-list syntax error tied to the offending source span.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LambdaListError {
    /// Error category.
    pub kind: LambdaListErrorKind,
    /// Source location of the error.
    pub span: Span,
}

impl LambdaListError {
    pub(crate) const fn expected_symbol(context: &'static str, span: Span) -> Self {
        Self {
            kind: LambdaListErrorKind::ExpectedSymbol { context },
            span,
        }
    }

    pub(crate) fn invalid(message: impl Into<String>, span: Span) -> Self {
        Self {
            kind: LambdaListErrorKind::InvalidForm {
                message: message.into(),
            },
            span,
        }
    }
}

impl fmt::Display for LambdaListErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedList => formatter.write_str("parameters must be a list"),
            Self::ExpectedSymbol { context } => write!(formatter, "{context} must be a symbol"),
            Self::InvalidForm { message } => formatter.write_str(message),
        }
    }
}

impl fmt::Display for LambdaListError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at byte {}..{}",
            self.kind, self.span.start, self.span.end
        )
    }
}

impl Error for LambdaListError {}
