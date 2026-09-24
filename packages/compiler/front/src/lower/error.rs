//! The failure type returned by AST-to-IR lowering.
//!
//! `LowerError` is owned by this crate, implements [`std::error::Error`]
//! directly, and names the form that has no `ncl-ir` representation rather than
//! erasing the reason behind a stringly typed box.

use crate::symbols::SymbolRef;

/// A reason an expression could not be lowered into `ncl-ir`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LowerError {
    /// The IR has no representation for this form yet.
    Unsupported {
        /// The Lisp name of the form.
        form: &'static str,
    },
    /// A lambda list uses a feature the lowering lane does not implement yet.
    UnsupportedLambdaList {
        /// The unimplemented lambda list feature.
        feature: &'static str,
    },
    /// A `go` or `return-from` targets a block or tag outside the function.
    EscapingControl {
        /// The block or tag name.
        name: SymbolRef,
    },
    /// The IR builder rejected an operation it was asked to append.
    Ir {
        /// The builder's message.
        detail: String,
    },
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported { form } => {
                write!(f, "no IR representation for {form}")
            }
            Self::UnsupportedLambdaList { feature } => {
                write!(f, "unsupported lambda list feature: {feature}")
            }
            Self::EscapingControl { name } => {
                write!(f, "{name} escapes the function being lowered")
            }
            Self::Ir { detail } => write!(f, "IR builder error: {detail}"),
        }
    }
}

impl std::error::Error for LowerError {}

impl From<String> for LowerError {
    fn from(detail: String) -> Self {
        Self::Ir { detail }
    }
}
