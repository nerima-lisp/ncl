//! The literal-constant type embedded in bytecode.

/// A literal value embedded directly in bytecode.
#[derive(Clone, Debug, PartialEq)]
pub enum Constant {
    /// The NCL empty-list value.
    Nil,
    /// A boolean literal.
    Boolean(bool),
    /// A signed integer literal.
    Integer(i64),
    /// A rational literal in normalized numerator/denominator form.
    Rational {
        /// Normalized numerator.
        numerator: i64,
        /// Positive denominator.
        denominator: i64,
    },
    /// A floating-point literal.
    Float(f64),
    /// A string literal.
    String(String),
    /// A character literal.
    Character(char),
    /// A package-resolved symbol name.
    Symbol(String),
    /// An escaped symbol name.
    SymbolExact(String),
    /// A package-resolved keyword name.
    Keyword(String),
    /// An escaped keyword name.
    KeywordExact(String),
}
