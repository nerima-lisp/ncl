//! Structural pieces shared by compound Common Lisp types.

use crate::ids::{CompilationUnitId, SymbolId};

/// A recognized atomic Common Lisp type name.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum BuiltinType {
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
}

impl BuiltinType {
    /// Resolves an upper-case Common Lisp type name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
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
            _ => return None, // check-added-lines: allow(wildcard) unmodeled names remain opaque
        })
    }
}

/// A compile-time object reference used by `MEMBER` and `EQL` types.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum MemberValue {
    /// The `NIL` symbol.
    Nil,
    /// The `T` symbol.
    True,
    /// A fixnum value.
    Integer(i64),
    /// A character value.
    Character(char),
    /// A single-float value stored by its IEEE 754 bits.
    SingleFloat(u32),
    /// A double-float value stored by its IEEE 754 bits.
    DoubleFloat(u64),
    /// A symbol identity.
    Symbol(SymbolId),
    /// An object whose identity is known but whose structure is not modeled.
    OpaqueObject {
        /// The compilation unit that owns this object identity.
        owner: CompilationUnitId,
        /// The object's stable index within that compilation unit.
        index: u64,
    },
}

impl MemberValue {
    pub(super) const fn canonicalized(self) -> Self {
        match self {
            Self::SingleFloat(bits) if bits.trailing_zeros() >= 31 => Self::SingleFloat(0),
            Self::DoubleFloat(bits) if bits.trailing_zeros() >= 63 => Self::DoubleFloat(0),
            _ => self, // check-added-lines: allow(wildcard) other member values need no canonicalization
        }
    }

    /// Compares values using the represented Common Lisp `EQL` cases.
    #[must_use]
    #[allow(clippy::float_cmp)]
    pub fn eql(self, other: Self) -> bool {
        match (self, other) {
            (Self::SingleFloat(left), Self::SingleFloat(right)) => {
                let left = f32::from_bits(left);
                let right = f32::from_bits(right);
                !left.is_nan() && !right.is_nan() && left == right
            }
            (Self::DoubleFloat(left), Self::DoubleFloat(right)) => {
                let left = f64::from_bits(left);
                let right = f64::from_bits(right);
                !left.is_nan() && !right.is_nan() && left == right
            }
            _ => self == other, // check-added-lines: allow(wildcard) nonfloat EQL cases use exact enum equality
        }
    }
}

/// The dimensional constraint of an array type.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ArrayDimensions {
    /// Any rank and extents.
    Any,
    /// Any extents at exactly this rank.
    Rank(usize),
    /// A fixed rank with independently unconstrained extents.
    Axes(Vec<Option<usize>>),
}

/// An array type's element, dimensions, and simplicity constraints.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ArrayType {
    /// Element type, or `None` for any element type.
    pub element_type: Option<crate::ids::TypeId>,
    /// Rank and extent constraints.
    pub dimensions: ArrayDimensions,
    /// Whether only simple arrays are accepted.
    pub simple: bool,
}

/// Parameter constraint of a function type.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum FunctionParameters {
    /// Any parameter list.
    Any,
    /// A fixed list of parameter types.
    Fixed(Vec<crate::ids::TypeId>),
}

/// A function's parameter and return type constraints.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FunctionType {
    /// Accepted parameter types.
    pub parameters: FunctionParameters,
    /// Type of returned values.
    pub returns: crate::ids::TypeId,
}
