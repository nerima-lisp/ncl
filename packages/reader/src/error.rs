//! Reader failure types.

use ncl_object::ObjectError;

/// A failure raised while reading a form.
///
/// Reader errors cover lexical and dispatch failures, dynamic-variable
/// violations, and object-layer failures surfaced while building the result.
/// The enum is `#[non_exhaustive]` so downstream lanes are not broken by a
/// later addition.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ReadError {
    /// Input ended before a complete form was read.
    UnexpectedEof,
    /// A `)` was read when no list was open.
    UnmatchedRightParen,
    /// A list ended with `.` but no cdr form followed.
    DotWithoutCdr,
    /// A token with more than one package marker, or an empty symbol name.
    InvalidSymbolToken(String),
    /// A package named by a token prefix does not exist.
    PackageNotFound(String),
    /// `*read-base*` or a `#nR` radix is outside the closed range `2..=36`.
    InvalidBase(u32),
    /// A digit is not valid for the requested radix.
    InvalidDigit(char),
    /// A token that looks numeric could not be parsed as a number.
    InvalidNumber(String),
    /// An integer is too large to represent in this build.
    NumberOutOfRange,
    /// A single-float was requested but is not representable by the object layer.
    FloatFormatUnavailable(char),
    /// `#.` appeared while `*read-eval*` is false.
    ReadEvalDisabled,
    /// `#.` appeared while `*read-eval*` is true but no evaluator is available.
    ReadEvalUnavailable,
    /// A `#\` character literal was malformed.
    InvalidCharacter,
    /// A `#\` character name is not recognized.
    UnknownCharacterName(String),
    /// A `#x` dispatch sub-character has no defined function.
    UndefinedDispatchMacro(char),
    /// A macro character has a user-defined function that cannot be invoked here.
    UninvocableMacroFunction(char),
    /// A feature expression was malformed.
    InvalidFeatureExpression,
    /// Array reader syntax is not available yet.
    ArraySyntax,
    /// Structure reader syntax is not available yet.
    StructureSyntax,
    /// Pathname reader syntax is not available yet.
    PathnameSyntax,
    /// A readtable operation used a non-dispatch macro character.
    NotDispatchMacro(char),
    /// An object-layer failure.
    Object(ObjectError),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected end of input"),
            Self::UnmatchedRightParen => f.write_str("unmatched right parenthesis"),
            Self::DotWithoutCdr => f.write_str("dot in list is not followed by a cdr form"),
            Self::InvalidSymbolToken(token) => {
                write!(f, "invalid symbol token: {token}")
            }
            Self::PackageNotFound(name) => write!(f, "package not found: {name}"),
            Self::InvalidBase(base) => write!(f, "invalid read base: {base}"),
            Self::InvalidDigit(digit) => write!(f, "invalid digit for base: {digit}"),
            Self::InvalidNumber(token) => write!(f, "invalid number token: {token}"),
            Self::NumberOutOfRange => f.write_str("number out of range"),
            Self::FloatFormatUnavailable(marker) => {
                write!(f, "float format is unavailable: {marker}")
            }
            Self::ReadEvalDisabled => f.write_str("#. requires *read-eval* to be true"),
            Self::ReadEvalUnavailable => f.write_str("#. has no evaluator available"),
            Self::InvalidCharacter => f.write_str("invalid character literal"),
            Self::UnknownCharacterName(name) => write!(f, "unknown character name: {name}"),
            Self::UndefinedDispatchMacro(ch) => write!(f, "undefined dispatch macro: #{ch}"),
            Self::UninvocableMacroFunction(ch) => {
                write!(f, "macro character {ch} has no invocable function")
            }
            Self::InvalidFeatureExpression => f.write_str("invalid feature expression"),
            Self::ArraySyntax => f.write_str("array reader syntax is unavailable"),
            Self::StructureSyntax => f.write_str("structure reader syntax is unavailable"),
            Self::PathnameSyntax => f.write_str("pathname reader syntax is unavailable"),
            Self::NotDispatchMacro(ch) => write!(f, "{ch} is not a dispatch macro character"),
            Self::Object(error) => write!(f, "object error: {error}"),
        }
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Object(error) = self {
            Some(error)
        } else {
            None
        }
    }
}

impl From<ObjectError> for ReadError {
    fn from(error: ObjectError) -> Self {
        Self::Object(error)
    }
}
