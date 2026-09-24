//! Symbols as the internal AST refers to them.
//!
//! The AST never stores an `ncl-object` `Word`, so a symbol is carried as its
//! home package name and its name. A macro expansion may also produce
//! uninterned symbols (`gensym`); those carry an expansion-local identity so
//! two distinct uninterned symbols with the same name stay distinct.

use std::fmt;

/// A reference to a symbol, independent of any heap value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SymbolRef {
    /// The home package name, or `None` for an uninterned symbol.
    pub package: Option<String>,
    /// The symbol's name.
    pub name: String,
    /// An expansion-local identity for an uninterned symbol.
    ///
    /// This is `Some` exactly when `package` is `None`. Two `SymbolRef` values
    /// with the same name and different identities denote distinct symbols.
    pub uninterned: Option<u32>,
}

impl SymbolRef {
    /// Build a reference to an interned symbol.
    #[must_use]
    pub fn interned(package: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            package: Some(package.into()),
            name: name.into(),
            uninterned: None,
        }
    }

    /// Build a reference to a symbol in the `KEYWORD` package.
    #[must_use]
    pub fn keyword(name: impl Into<String>) -> Self {
        Self::interned("KEYWORD", name)
    }

    /// Build a reference to an uninterned symbol with an expansion-local identity.
    #[must_use]
    pub fn uninterned(name: impl Into<String>, identity: u32) -> Self {
        Self {
            package: None,
            name: name.into(),
            uninterned: Some(identity),
        }
    }

    /// Whether this symbol has no home package.
    #[must_use]
    pub const fn is_uninterned(&self) -> bool {
        self.package.is_none()
    }

    /// Whether this symbol is interned in the `KEYWORD` package.
    #[must_use]
    pub fn is_keyword(&self) -> bool {
        self.package.as_deref() == Some("KEYWORD")
    }

    /// The home package name, if any.
    #[must_use]
    pub fn package_name(&self) -> Option<&str> {
        self.package.as_deref()
    }

    /// Whether this is the symbol named `name` in `package`.
    #[must_use]
    pub fn is_named(&self, package: &str, name: &str) -> bool {
        self.name == name && self.package.as_deref() == Some(package)
    }

    /// Whether this is the symbol `name` in the `COMMON-LISP` package.
    #[must_use]
    pub fn is_common_lisp(&self, name: &str) -> bool {
        self.is_named("COMMON-LISP", name)
    }
}

impl fmt::Display for SymbolRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.package, self.uninterned) {
            (Some(package), _) => write!(f, "{package}:{}", self.name),
            (None, Some(identity)) => write!(f, "#:{}#{}", self.name, identity),
            (None, None) => write!(f, "#:{}", self.name),
        }
    }
}
