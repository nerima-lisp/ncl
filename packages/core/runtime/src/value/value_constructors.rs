use std::cell::RefCell;
use std::rc::Rc;

use super::{Environment, Rational, RuntimeError, Value};

mod stream_constructors;

impl Value {
    pub(crate) fn mutable_string(value: String) -> Self {
        Self::MutableString(Rc::new(RefCell::new(value)))
    }

    /// Converts a Rust boolean to the Lisp truth value representation.
    ///
    /// `false` is represented by `NIL`; `true` is represented by `T`.
    #[must_use]
    pub const fn boolean(value: bool) -> Self {
        if value {
            Self::Boolean(true)
        } else {
            Self::Nil
        }
    }

    /// Creates a string value from an owned or reference-counted string.
    pub fn string(value: impl Into<Rc<str>>) -> Self {
        Self::String(value.into())
    }

    /// Creates an integer value, demoting back to a machine integer when the
    /// arbitrary-precision value still fits in `i64` (e.g. after a bignum
    /// division reduces the magnitude).
    pub(crate) fn big_integer(value: ibig::IBig) -> Self {
        i64::try_from(&value).map_or_else(|_| Self::BigInteger(Rc::new(value)), Self::Integer)
    }

    pub(crate) fn rational(numerator: i128, denominator: i128) -> Result<Self, RuntimeError> {
        let rational = Rational::new(numerator, denominator)?;
        if rational.denominator() == &ibig::IBig::from(1) {
            Ok(Self::big_integer(rational.numerator().clone()))
        } else {
            Ok(Self::Rational(rational))
        }
    }

    pub(crate) fn rational_big(
        numerator: ibig::IBig,
        denominator: ibig::IBig,
    ) -> Result<Self, RuntimeError> {
        let rational = Rational::new_big(numerator, denominator)?;
        if rational.denominator() == &ibig::IBig::from(1) {
            Ok(Self::big_integer(rational.numerator().clone()))
        } else {
            Ok(Self::Rational(rational))
        }
    }

    /// Creates a package designator with the supplied name.
    pub fn package(value: impl AsRef<str>) -> Self {
        Self::Package(Rc::from(value.as_ref()))
    }

    pub(crate) const fn environment(value: Environment) -> Self {
        Self::Environment(value)
    }

    /// Creates a case-normalized symbol.
    pub fn symbol(value: impl AsRef<str>) -> Self {
        Self::Symbol(Rc::from(value.as_ref().to_ascii_uppercase().as_str()))
    }

    /// Creates a symbol while preserving the supplied spelling.
    pub fn symbol_exact(value: impl AsRef<str>) -> Self {
        Self::SymbolExact(Rc::from(value.as_ref()))
    }

    /// Creates an uninterned symbol.
    pub fn uninterned_symbol(value: impl AsRef<str>) -> Self {
        Self::UninternedSymbol(Rc::from(value.as_ref()))
    }

    /// Creates a case-normalized keyword, accepting an optional leading colon.
    pub fn keyword(value: impl AsRef<str>) -> Self {
        let value = value.as_ref().trim_start_matches(':').to_ascii_uppercase();
        Self::Keyword(Rc::from(value))
    }

    /// Creates a keyword while preserving its spelling, apart from a leading colon.
    pub fn keyword_exact(value: impl AsRef<str>) -> Self {
        Self::KeywordExact(Rc::from(value.as_ref().trim_start_matches(':')))
    }

    /// Creates a proper list, using `NIL` for an empty list.
    #[must_use]
    pub fn list(values: Vec<Self>) -> Self {
        if values.is_empty() {
            Self::Nil
        } else {
            Self::List(Rc::new(values))
        }
    }

    /// Creates a mutable cons cell.
    #[must_use]
    pub fn cons_cell(car: Self, cdr: Self) -> Self {
        Self::MutableCons(std::rc::Rc::new(std::cell::RefCell::new((car, cdr))))
    }

    /// Creates a dotted list from its proper prefix and tail.
    #[must_use]
    pub fn dotted_list(items: Vec<Self>, tail: Self) -> Self {
        Self::DottedList {
            items: Rc::new(items),
            tail: Rc::new(tail),
        }
    }

    /// Creates a vector value.
    #[must_use]
    pub fn vector(values: Vec<Self>) -> Self {
        Self::Vector(Rc::new(super::VectorData {
            elements: Rc::new(RefCell::new(values)),
            metadata: RefCell::new(super::ArrayMetadata {
                element_type: Self::symbol("T"),
                adjustable: false,
                fill_pointer: None,
                displaced_to: None,
                displaced_to_value: None,
                displaced_index_offset: 0,
            }),
        }))
    }

    /// Creates an array value with explicit dimensions and row-major elements.
    #[must_use]
    pub fn array(dimensions: Vec<usize>, elements: Vec<Self>) -> Self {
        Self::Array {
            dimensions: Rc::new(dimensions),
            elements: Rc::new(RefCell::new(elements)),
            metadata: Rc::new(RefCell::new(super::ArrayMetadata {
                element_type: Self::symbol("T"),
                adjustable: false,
                fill_pointer: None,
                displaced_to: None,
                displaced_to_value: None,
                displaced_index_offset: 0,
            })),
        }
    }

    pub(crate) fn hash_table(test: impl AsRef<str>) -> Self {
        Self::HashTable {
            test: Rc::from(test.as_ref()),
            entries: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub(crate) fn values(values: Vec<Self>) -> Self {
        Self::Values(Rc::new(values))
    }
}

#[cfg(test)]
mod tests {
    use super::Value;

    #[test]
    fn rational_constructor_reduces_integral_values_to_integers() {
        let cases = [(2, 1, 2), (-6, -3, 2)];

        for (numerator, denominator, expected) in cases {
            assert!(matches!(
                Value::rational(numerator, denominator),
                Ok(Value::Integer(value)) if value == expected
            ));
        }
    }
}
