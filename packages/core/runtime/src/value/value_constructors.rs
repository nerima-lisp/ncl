use std::cell::RefCell;
use std::rc::Rc;

use super::{
    BigRational, Environment, PackageObject, Rational, RuntimeError, SharedCons, SharedElements,
    SymbolObject, Value,
};

mod stream_constructors;

impl Value {
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

    pub(crate) fn mutable_string(value: impl AsRef<str>) -> Self {
        Self::string(value.as_ref())
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

    pub(crate) fn big_rational(
        numerator: ibig::IBig,
        denominator: ibig::IBig,
    ) -> Result<Self, RuntimeError> {
        let rational = BigRational::new(numerator, denominator)?;
        if rational.denominator() == &ibig::IBig::from(1) {
            Ok(Self::big_integer(rational.numerator().clone()))
        } else {
            Ok(Self::BigRational(Rc::new(rational)))
        }
    }

    /// Creates a package designator with the supplied name.
    pub fn package(value: impl AsRef<str>) -> Self {
        Self::Package(Rc::from(value.as_ref()))
    }

    /// Creates a value backed by a stable runtime package object.
    pub(crate) fn package_object(value: PackageObject) -> Self {
        Self::PackageObject(value)
    }

    pub(crate) const fn environment(value: Environment) -> Self {
        Self::Environment(value)
    }

    /// Creates a case-normalized symbol.
    pub fn symbol(value: impl AsRef<str>) -> Self {
        Self::Symbol(Rc::from(value.as_ref().to_ascii_uppercase().as_str()))
    }

    pub(crate) fn interned_symbol(value: SymbolObject) -> Self {
        Self::InternedSymbol(value)
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
        Self::dotted_list(values, Self::Nil)
    }

    /// Creates a dotted list from its proper prefix and tail.
    #[must_use]
    pub fn dotted_list(items: Vec<Self>, tail: Self) -> Self {
        items
            .into_iter()
            .rev()
            .fold(tail, |cdr, car| Self::cons(car, cdr))
    }

    /// Allocates one cons, preserving the supplied CAR and CDR identities.
    #[must_use]
    pub fn cons(car: Self, cdr: Self) -> Self {
        Self::Cons(SharedCons::new(car, cdr))
    }

    /// Creates a vector value.
    #[must_use]
    pub fn vector(values: Vec<Self>) -> Self {
        Self::Vector(SharedElements::new(values))
    }

    pub(crate) fn vector_with_elements(elements: SharedElements) -> Self {
        Self::Vector(elements)
    }

    /// Creates an array value with explicit dimensions and row-major elements.
    #[must_use]
    pub fn array(dimensions: Vec<usize>, elements: Vec<Self>) -> Self {
        Self::Array {
            dimensions: Rc::new(dimensions),
            elements: SharedElements::new(elements),
        }
    }

    pub(crate) fn array_with_elements(dimensions: Vec<usize>, elements: SharedElements) -> Self {
        Self::Array {
            dimensions: Rc::new(dimensions),
            elements,
        }
    }

    pub(crate) fn hash_table(test: impl AsRef<str>) -> Self {
        Self::hash_table_with_options(test, 16, Self::Float(1.5), Self::Float(1.0), None)
    }

    pub(crate) fn hash_table_with_options(
        test: impl AsRef<str>,
        size: usize,
        rehash_size: Self,
        rehash_threshold: Self,
        weakness: Option<Rc<str>>,
    ) -> Self {
        Self::HashTable {
            test: Rc::from(test.as_ref()),
            size: Rc::new(std::cell::Cell::new(size)),
            rehash_size: Rc::new(rehash_size),
            rehash_threshold: Rc::new(rehash_threshold),
            weakness,
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
