use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use super::{Environment, Rational, RuntimeError, Stream, Value};

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

    pub(crate) fn rational(numerator: i128, denominator: i128) -> Result<Self, RuntimeError> {
        let rational = Rational::new(numerator, denominator)?;
        if rational.denominator() == 1 {
            Ok(Self::Integer(rational.numerator()))
        } else {
            Ok(Self::Rational(rational))
        }
    }

    pub(crate) fn string_input_stream(source: &str, start: usize, end: usize) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::input(source, start, end))))
    }

    pub(crate) fn string_output_stream() -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::output())))
    }

    pub(crate) fn file_input_stream(source: &str) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_input(source))))
    }

    pub(crate) fn file_output_stream(path: PathBuf, initial: String) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_output(path, initial))))
    }

    pub(crate) fn file_io_stream(path: PathBuf, source: &str, append: bool) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_io(path, source, append))))
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
        Self::Vector(Rc::new(values))
    }

    /// Creates an array value with explicit dimensions and row-major elements.
    #[must_use]
    pub fn array(dimensions: Vec<usize>, elements: Vec<Self>) -> Self {
        Self::Array {
            dimensions: Rc::new(dimensions),
            elements: Rc::new(elements),
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
