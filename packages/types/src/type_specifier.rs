//! Value types representing parsed Common Lisp type specifiers.

use ncl_object::Word;

/// A parsed Common Lisp type specifier.
///
/// [`TypeSpecifier`] is a value object produced by
/// [`parse_type_specifier`](crate::parse_type_specifier) and consumed by
/// [`typep`](crate::typep()) and [`subtypep`](crate::subtypep()). Variants that
/// carry heap [`Word`]s (`IntegerRange`, `Member`, `Eql`, `Satisfies`,
/// `Deftype`) reference the objects of the source form; a parsed specifier
/// must not outlive the root protection of that form.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum TypeSpecifier {
    /// An atomic type name such as `integer` or `string`.
    Named(NamedType),
    /// `(integer low high)`; `None` marks an unbounded (`*`) end.
    IntegerRange {
        /// Lower bound; `None` means `*`.
        low: Option<Word>,
        /// Upper bound; `None` means `*`.
        high: Option<Word>,
    },
    /// `(or spec*)`.
    Or(Vec<Self>),
    /// `(and spec*)`.
    And(Vec<Self>),
    /// `(not spec)`.
    Not(Box<Self>),
    /// `(member object*)`.
    Member(Vec<Word>),
    /// `(eql object)`.
    Eql(Word),
    /// `(satisfies predicate)`.
    Satisfies(Word),
    /// `(array element-type dimensions)` and `(simple-array ...)`.
    Array {
        /// Element type; `None` matches any element type.
        element_type: Option<Box<Self>>,
        /// Dimension specifier; `None` matches any dimensions.
        dimensions: Option<ArrayDimensions>,
        /// Whether the `simple-array` form was used.
        simple: bool,
    },
    /// `(vector element-type size)`.
    Vector {
        /// Element type; `None` matches any element type.
        element_type: Option<Box<Self>>,
        /// Fixed length; `None` matches any length.
        size: Option<Word>,
    },
    /// `(cons car-type cdr-type)`.
    Cons {
        /// Type of the `car`.
        car: Box<Self>,
        /// Type of the `cdr`.
        cdr: Box<Self>,
    },
    /// `(function lambda-list return-type)`.
    Function {
        /// Parameter types; empty means `*` (any signature).
        lambda_list: Vec<Self>,
        /// Return type.
        return_type: Box<Self>,
    },
    /// `(values spec*)`.
    Values(Vec<Self>),
    /// A `deftype` name awaiting expansion.
    Deftype {
        /// The `deftype` name symbol.
        name: Word,
        /// Arguments passed to the `deftype` definition.
        args: Vec<Word>,
    },
}

/// The dimensions part of an `(array ...)` or `(simple-array ...)` specifier.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ArrayDimensions {
    /// `*`: any rank.
    Wild,
    /// A dimension list such as `(3 4)` or `(* 4)`; `None` marks `*`.
    Ranks(Vec<Option<Word>>),
    /// A single rank, as in `(array t 3)`.
    Rank(usize),
}

/// Atomic standard type names.
///
/// The variants cover the standard atomic type specifiers that this phase
/// recognizes. Types with no distinct object representation
/// (`short-float`, `single-float`, `long-float`, and the character and
/// base-string families) parse but never match in [`typep`](crate::typep()).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NamedType {
    /// The universal type.
    T,
    /// The empty type.
    Nil,
    /// `boolean`.
    Boolean,
    /// `symbol`.
    Symbol,
    /// `keyword`.
    Keyword,
    /// `package`.
    Package,
    /// `function`.
    Function,
    /// `compiled-function`.
    CompiledFunction,
    /// `cons`.
    Cons,
    /// `list`.
    List,
    /// `null`.
    Null,
    /// `atom`.
    Atom,
    /// `number`.
    Number,
    /// `real`.
    Real,
    /// `rational`.
    Rational,
    /// `integer`.
    Integer,
    /// `fixnum`.
    Fixnum,
    /// `bignum`.
    Bignum,
    /// `ratio`.
    Ratio,
    /// `float`.
    Float,
    /// `short-float`.
    ShortFloat,
    /// `single-float`.
    SingleFloat,
    /// `double-float`.
    DoubleFloat,
    /// `long-float`.
    LongFloat,
    /// `complex`.
    Complex,
    /// `character`.
    Character,
    /// `base-char`.
    BaseChar,
    /// `standard-char`.
    StandardChar,
    /// `extended-char`.
    ExtendedChar,
    /// `string`.
    String,
    /// `base-string`.
    BaseString,
    /// `simple-string`.
    SimpleString,
    /// `simple-base-string`.
    SimpleBaseString,
    /// `bit-vector`.
    BitVector,
    /// `simple-bit-vector`.
    SimpleBitVector,
    /// `vector`.
    Vector,
    /// `simple-vector`.
    SimpleVector,
    /// `array`.
    Array,
    /// `simple-array`.
    SimpleArray,
    /// `sequence`.
    Sequence,
    /// `hash-table`.
    HashTable,
    /// `stream`.
    Stream,
    /// `random-state`.
    RandomState,
    /// `restart`.
    Restart,
    /// `structure-object`.
    Structure,
    /// The `(values ...)` return-type specifier.
    ValuesType,
}

impl NamedType {
    /// Resolve an upper-case Common Lisp type name to its built-in variant.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "T" => Self::T,
            "NIL" => Self::Nil,
            "BOOLEAN" => Self::Boolean,
            "SYMBOL" => Self::Symbol,
            "KEYWORD" => Self::Keyword,
            "PACKAGE" => Self::Package,
            "FUNCTION" => Self::Function,
            "COMPILED-FUNCTION" => Self::CompiledFunction,
            "CONS" => Self::Cons,
            "LIST" => Self::List,
            "NULL" => Self::Null,
            "ATOM" => Self::Atom,
            "NUMBER" => Self::Number,
            "REAL" => Self::Real,
            "RATIONAL" => Self::Rational,
            "INTEGER" => Self::Integer,
            "FIXNUM" => Self::Fixnum,
            "BIGNUM" => Self::Bignum,
            "RATIO" => Self::Ratio,
            "FLOAT" => Self::Float,
            "SHORT-FLOAT" => Self::ShortFloat,
            "SINGLE-FLOAT" => Self::SingleFloat,
            "DOUBLE-FLOAT" => Self::DoubleFloat,
            "LONG-FLOAT" => Self::LongFloat,
            "COMPLEX" => Self::Complex,
            "CHARACTER" => Self::Character,
            "BASE-CHAR" => Self::BaseChar,
            "STANDARD-CHAR" => Self::StandardChar,
            "EXTENDED-CHAR" => Self::ExtendedChar,
            "STRING" => Self::String,
            "BASE-STRING" => Self::BaseString,
            "SIMPLE-STRING" => Self::SimpleString,
            "SIMPLE-BASE-STRING" => Self::SimpleBaseString,
            "BIT-VECTOR" => Self::BitVector,
            "SIMPLE-BIT-VECTOR" => Self::SimpleBitVector,
            "VECTOR" => Self::Vector,
            "SIMPLE-VECTOR" => Self::SimpleVector,
            "ARRAY" => Self::Array,
            "SIMPLE-ARRAY" => Self::SimpleArray,
            "SEQUENCE" => Self::Sequence,
            "HASH-TABLE" => Self::HashTable,
            "STREAM" => Self::Stream,
            "RANDOM-STATE" => Self::RandomState,
            "RESTART" => Self::Restart,
            "STRUCTURE-OBJECT" => Self::Structure,
            "VALUES" => Self::ValuesType,
            _ => return None,
        })
    }
}
