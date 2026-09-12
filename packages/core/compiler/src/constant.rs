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
    /// An integer literal too large for `i64`, stored as its validated
    /// decimal digits (optionally sign-prefixed). Parsed into an
    /// arbitrary-precision integer at the point the constant is loaded into
    /// a `Value`, since `ncl-compiler` itself has no bignum dependency.
    BigInteger(String),
    /// A rational literal in normalized numerator/denominator form.
    Rational {
        /// Normalized numerator.
        numerator: i64,
        /// Positive denominator.
        denominator: i64,
    },
    /// A rational literal whose components do not fit in `i64`, stored as
    /// validated decimal digits and parsed by the runtime.
    BigRational {
        /// Decimal numerator.
        numerator: String,
        /// Decimal positive denominator.
        denominator: String,
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
