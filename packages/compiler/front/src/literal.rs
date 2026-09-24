//! Readable data that the AST can carry without holding a heap value.
//!
//! `Literal` mirrors the object model's readable surface structurally, so
//! `quote`d data and self-evaluating atoms survive independently of the heap.
//! It is the AST counterpart of `ncl_ir::Constant` plus the structural data
//! that `Constant::Object` defers to the constant table.

use crate::symbols::SymbolRef;

/// A numeric literal.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum NumberLiteral {
    /// An integer that fits the fixnum range.
    Fixnum(i64),
    /// An integer wider than a fixnum, as little-endian 32-bit limbs.
    Bignum {
        /// Whether the value is negative.
        negative: bool,
        /// Little-endian limbs, least significant first.
        limbs: Vec<u32>,
    },
    /// A ratio of two integers.
    Ratio {
        /// The numerator.
        numerator: Box<Self>,
        /// The denominator.
        denominator: Box<Self>,
    },
    /// A single-precision float.
    SingleFloat(f32),
    /// A double-precision float.
    DoubleFloat(f64),
    /// A complex number.
    Complex {
        /// The real part.
        real: Box<Self>,
        /// The imaginary part.
        imaginary: Box<Self>,
    },
}

/// A datum that the AST can represent without holding a heap value.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Literal {
    /// The empty list, `nil`.
    Nil,
    /// The canonical true value, `t`.
    T,
    /// A symbol.
    Symbol(SymbolRef),
    /// A number.
    Number(NumberLiteral),
    /// A character, as a Unicode scalar value.
    Character(u32),
    /// A string, as Unicode scalar values.
    String(Vec<char>),
    /// A cons cell.
    Cons(Box<Self>, Box<Self>),
    /// A simple vector.
    Vector(Vec<Self>),
    /// A general array, in row-major order.
    Array {
        /// The dimensions, most significant first.
        dimensions: Vec<usize>,
        /// The row-major elements.
        elements: Vec<Self>,
    },
    /// A bit vector, one boolean per element.
    BitVector(Vec<bool>),
}

impl Literal {
    /// Build a fixnum literal.
    #[must_use]
    pub const fn fixnum(value: i64) -> Self {
        Self::Number(NumberLiteral::Fixnum(value))
    }

    /// Build a double-float literal.
    #[must_use]
    pub const fn double(value: f64) -> Self {
        Self::Number(NumberLiteral::DoubleFloat(value))
    }

    /// Whether this datum evaluates to itself when it appears as a form.
    #[must_use]
    pub const fn is_self_evaluating(&self) -> bool {
        !matches!(self, Self::Symbol(_) | Self::Cons(_, _))
    }

    /// Collect a proper list of data into a vector.
    ///
    /// Returns `None` for a dotted list; callers turn that into
    /// [`FrontError::ImproperList`](crate::FrontError::ImproperList).
    #[must_use]
    pub fn list_elements(&self) -> Option<Vec<&Self>> {
        let mut elements = Vec::new();
        let mut cursor = self;
        loop {
            match cursor {
                Self::Nil => return Some(elements),
                Self::Cons(car, cdr) => {
                    elements.push(car);
                    cursor = cdr;
                }
                _ => return None,
            }
        }
    }
}
