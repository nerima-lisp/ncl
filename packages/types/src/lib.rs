//! Type specifiers, predicates, and type errors.
//!
//! This crate parses Common Lisp type specifiers into [`TypeSpecifier`] value
//! objects and answers [`typep()`] and [`subtypep()`] queries over them. It also
//! registers the standard type names, class names, and type-related functions
//! it owns ([`register()`]) into a [`ncl_object::Runtime`].

mod adapter;
pub mod error;
pub mod parse;
pub mod register;
pub mod subtypep;
pub(crate) mod text;
pub mod type_specifier;
pub mod typep;

pub use error::TypeError;
pub use parse::parse_type_specifier;
pub use register::register;
pub use subtypep::subtypep;
pub use type_specifier::{
    ArrayDimension, ArrayDimensions, IntegerBound, NamedType, TypeSpecifier, Value,
};
pub use typep::typep;

/// Serialize a type-specifier value at the object-layer boundary.
pub use adapter::serialize_value;
