//! A placeholder type specifier until `ncl-types` freezes its public API.
//!
//! The `ncl-types` crate is a Wave 1 peer lane and had not landed its API when
//! this contract was frozen. `TypeSpecifier` therefore carries the source form
//! of the type specifier as a [`Literal`], which loses no information: a symbol
//! or a compound list both survive. When `ncl_types::TypeSpecifier` lands, this
//! newtype is replaced by it and the conversion happens at the `form` boundary.

use crate::literal::Literal;

/// A type specifier as it appears in `the` and in `type` declarations.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeSpecifier {
    /// The source form of the type specifier.
    pub form: Literal,
}

impl TypeSpecifier {
    /// Wrap a source form as a type specifier.
    #[must_use]
    pub const fn new(form: Literal) -> Self {
        Self { form }
    }

    /// Borrow the underlying source form.
    #[must_use]
    pub const fn form(&self) -> &Literal {
        &self.form
    }

    /// Consume the specifier and return its source form.
    #[must_use]
    pub fn into_form(self) -> Literal {
        self.form
    }
}
