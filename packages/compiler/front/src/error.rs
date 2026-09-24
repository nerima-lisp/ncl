//! The single failure type returned by the front end.
//!
//! `FrontError` is owned by this crate, implements [`std::error::Error`]
//! directly, and keeps the object layer's failure as a distinct variant rather
//! than erasing it behind `Box<dyn Error>`.

use ncl_object::ObjectError;

use crate::symbols::SymbolRef;

/// A failure while reading, parsing, expanding, or declaring a form.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FrontError {
    /// A list form was improper (dotted) where a proper list is required.
    ImproperList,
    /// A special form received the wrong number of subforms.
    WrongNumberOfForms {
        /// The operator symbol.
        operator: SymbolRef,
        /// A description of the accepted shape.
        expected: &'static str,
        /// The number of subforms actually present.
        found: usize,
    },
    /// A lambda list keyword appeared out of the permitted order.
    LambdaListOrder {
        /// The offending keyword.
        keyword: SymbolRef,
    },
    /// A lambda list keyword appeared more than once.
    DuplicateLambdaListKeyword {
        /// The repeated keyword.
        keyword: SymbolRef,
    },
    /// An unrecognised lambda list keyword.
    UnknownLambdaListKeyword {
        /// The offending keyword.
        keyword: SymbolRef,
    },
    /// A binding, parameter, or block name appeared more than once.
    DuplicateName {
        /// The repeated name.
        name: SymbolRef,
    },
    /// A declaration form was malformed.
    MalformedDeclaration {
        /// A description of the malformed declaration.
        detail: String,
    },
    /// A quoted object cannot be represented by the frozen literal set.
    UnsupportedLiteral,
    /// A macro call failed.
    MacroExpansion {
        /// The macro name.
        name: SymbolRef,
        /// A description of the failure.
        detail: String,
    },
    /// A form appeared where an operator was expected.
    InvalidOperator {
        /// A description of the offending form.
        detail: String,
    },
    /// A special form is malformed in a way other than its argument count.
    MalformedForm {
        /// The operator symbol.
        operator: SymbolRef,
        /// A description of what is malformed.
        detail: String,
    },
    /// The object layer rejected a form while it was being read.
    Object(ObjectError),
}

impl std::fmt::Display for FrontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ImproperList => f.write_str("improper list where a proper list is required"),
            Self::WrongNumberOfForms {
                operator,
                expected,
                found,
            } => write!(f, "{operator} expects {expected}, found {found} subforms"),
            Self::LambdaListOrder { keyword } => {
                write!(f, "lambda list keyword {keyword} is out of order")
            }
            Self::DuplicateLambdaListKeyword { keyword } => {
                write!(f, "lambda list keyword {keyword} appears more than once")
            }
            Self::UnknownLambdaListKeyword { keyword } => {
                write!(f, "unknown lambda list keyword {keyword}")
            }
            Self::DuplicateName { name } => write!(f, "duplicate name {name}"),
            Self::MalformedDeclaration { detail } => {
                write!(f, "malformed declaration: {detail}")
            }
            Self::UnsupportedLiteral => {
                f.write_str("quoted object is not representable in the literal set")
            }
            Self::MacroExpansion { name, detail } => {
                write!(f, "macro expansion of {name} failed: {detail}")
            }
            Self::InvalidOperator { detail } => write!(f, "invalid operator: {detail}"),
            Self::MalformedForm { operator, detail } => {
                write!(f, "malformed {operator} form: {detail}")
            }
            Self::Object(error) => write!(f, "object layer error: {error}"),
        }
    }
}

impl std::error::Error for FrontError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Object(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ObjectError> for FrontError {
    fn from(error: ObjectError) -> Self {
        Self::Object(error)
    }
}
