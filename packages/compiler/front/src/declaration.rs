//! Declaration specifiers and their parsed form.
//!
//! A declaration is a `(declare specifier*)` form whose specifiers are drawn
//! from the standard set. An unrecognised specifier is retained as
//! [`Declaration::Unknown`] rather than rejected, because Common Lisp requires
//! an implementation to ignore declarations it does not understand.

use crate::error::FrontError;
use crate::literal::Literal;
use crate::symbols::SymbolRef;
use crate::types::TypeSpecifier;

/// One declaration specifier.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Declaration {
    /// `(special name*)`.
    Special(Vec<SymbolRef>),
    /// `(type type name*)`.
    Type {
        /// The declared type.
        type_specifier: TypeSpecifier,
        /// The declared variables.
        names: Vec<SymbolRef>,
    },
    /// `(ftype type name)`.
    Ftype {
        /// The declared function type.
        type_specifier: TypeSpecifier,
        /// The declared function name.
        name: SymbolRef,
    },
    /// `(inline name*)`.
    Inline(Vec<SymbolRef>),
    /// `(notinline name*)`.
    Notinline(Vec<SymbolRef>),
    /// `(optimize quality*)`.
    Optimize(Vec<OptimizeQuality>),
    /// `(dynamic-extent name*)`.
    DynamicExtent(Vec<SymbolRef>),
    /// `(ignore name*)`.
    Ignore(Vec<SymbolRef>),
    /// `(ignorable name*)`.
    Ignorable(Vec<SymbolRef>),
    /// A specifier the front end does not interpret.
    Unknown {
        /// The declaration name.
        name: SymbolRef,
        /// The remaining arguments, kept as data.
        arguments: Vec<Literal>,
    },
}

/// One quality named by an `optimize` declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OptimizeQuality {
    /// The quality being set.
    pub quality: Quality,
    /// The value, in the range 0 through 3.
    pub value: u8,
}

/// The optimization qualities named by `optimize`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Quality {
    /// `speed`.
    Speed,
    /// `space`.
    Space,
    /// `safety`.
    Safety,
    /// `debug`.
    Debug,
    /// `compilation-speed`.
    CompilationSpeed,
    /// A quality name this front end does not interpret.
    Unknown,
}

impl Quality {
    /// Resolve a quality name, if it is one of the standard qualities.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "SPEED" => Self::Speed,
            "SPACE" => Self::Space,
            "SAFETY" => Self::Safety,
            "DEBUG" => Self::Debug,
            "COMPILATION-SPEED" => Self::CompilationSpeed,
            _ => return None,
        })
    }
}

impl Declaration {
    /// Whether this specifier names a variable as special.
    #[must_use]
    pub const fn is_special(&self) -> bool {
        matches!(self, Self::Special(_))
    }

    /// Parse one declaration specifier from its datum.
    ///
    /// # Errors
    ///
    /// Returns [`FrontError::MalformedDeclaration`] when the specifier is not a
    /// proper list headed by a symbol, or when a recognised specifier has the
    /// wrong shape.
    pub fn parse(specifier: &Literal) -> Result<Self, FrontError> {
        let elements = specifier
            .list_elements()
            .ok_or_else(|| malformed("specifier is not a proper list"))?;
        let Some((head, arguments)) = elements.split_first() else {
            return Err(malformed("empty specifier"));
        };
        let Literal::Symbol(name) = head else {
            return Err(malformed("specifier does not start with a symbol"));
        };
        match name.name.as_str() {
            "SPECIAL" => Ok(Self::Special(symbol_list(arguments)?)),
            "TYPE" => {
                let (type_specifier, names) = arguments
                    .split_first()
                    .ok_or_else(|| malformed("type declaration has no type"))?;
                Ok(Self::Type {
                    type_specifier: TypeSpecifier::new((*type_specifier).clone()),
                    names: symbol_list(names)?,
                })
            }
            "FTYPE" => {
                let (type_specifier, names) = arguments
                    .split_first()
                    .ok_or_else(|| malformed("ftype declaration has no type"))?;
                let Some(name) = names.first() else {
                    return Err(malformed("ftype declaration has no name"));
                };
                Ok(Self::Ftype {
                    type_specifier: TypeSpecifier::new((*type_specifier).clone()),
                    name: literal_symbol(name)?,
                })
            }
            "INLINE" => Ok(Self::Inline(symbol_list(arguments)?)),
            "NOTINLINE" => Ok(Self::Notinline(symbol_list(arguments)?)),
            "DYNAMIC-EXTENT" => Ok(Self::DynamicExtent(symbol_list(arguments)?)),
            "IGNORE" => Ok(Self::Ignore(symbol_list(arguments)?)),
            "IGNORABLE" => Ok(Self::Ignorable(symbol_list(arguments)?)),
            "OPTIMIZE" => Ok(Self::Optimize(optimize_qualities(arguments)?)),
            _ => Ok(Self::Unknown {
                name: name.clone(),
                arguments: arguments.iter().map(|item| (*item).clone()).collect(),
            }),
        }
    }
}

/// Parse the specifiers of a `(declare specifier*)` form.
///
/// # Errors
///
/// Returns [`FrontError::MalformedDeclaration`] when the form is not headed by
/// `declare`, or when a specifier is malformed.
pub fn parse_declare_form(form: &Literal) -> Result<Vec<Declaration>, FrontError> {
    let elements = form
        .list_elements()
        .ok_or_else(|| malformed("declare form is not a proper list"))?;
    let Some((head, specifiers)) = elements.split_first() else {
        return Err(malformed("empty declare form"));
    };
    if !is_named(head, "DECLARE") {
        return Err(malformed("form does not start with declare"));
    }
    specifiers
        .iter()
        .map(|specifier| Declaration::parse(specifier))
        .collect()
}

/// Build a malformed-declaration failure.
fn malformed(detail: &str) -> FrontError {
    FrontError::MalformedDeclaration {
        detail: detail.to_owned(),
    }
}

/// Whether a datum is the symbol `name`.
fn is_named(literal: &Literal, name: &str) -> bool {
    matches!(literal, Literal::Symbol(symbol) if symbol.name == name)
}

/// Extract a symbol from a datum.
fn literal_symbol(literal: &Literal) -> Result<SymbolRef, FrontError> {
    match literal {
        Literal::Symbol(symbol) => Ok(symbol.clone()),
        _ => Err(malformed("expected a symbol")),
    }
}

/// Extract a list of symbols.
fn symbol_list(literals: &[&Literal]) -> Result<Vec<SymbolRef>, FrontError> {
    literals.iter().map(|item| literal_symbol(item)).collect()
}

/// Parse the qualities of an `optimize` declaration.
fn optimize_qualities(literals: &[&Literal]) -> Result<Vec<OptimizeQuality>, FrontError> {
    literals
        .iter()
        .map(|item| {
            let elements = item
                .list_elements()
                .ok_or_else(|| malformed("optimize quality is not a list"))?;
            let [quality, value] = elements.as_slice() else {
                return Err(malformed("optimize quality needs a name and a value"));
            };
            let quality = literal_symbol(quality)?;
            let quality = Quality::from_name(&quality.name).unwrap_or(Quality::Unknown);
            let Literal::Number(crate::literal::NumberLiteral::Fixnum(value)) = value else {
                return Err(malformed("optimization quality value is not an integer"));
            };
            let value =
                u8::try_from(*value).map_err(|_| malformed("quality value out of range"))?;
            Ok(OptimizeQuality { quality, value })
        })
        .collect()
}
