//! The compiler-macro registry.
//!
//! A compiler macro rewrites a call before lowering. The registry stores, per
//! entry, the symbol descriptor, an arity pattern, an expansion callback, and
//! an optional feature bit, matching the front-end contract. Compiler macros
//! are consulted only when the head symbol is not already a macro.

use crate::error::FrontError;
use crate::literal::Literal;
use crate::symbols::SymbolRef;

/// Expands a compiler-macro call.
///
/// The callback receives the whole call form as a datum and returns the
/// replacement, or `None` when it declines to expand. Returning a datum rather
/// than an AST keeps the registry independent of the expander.
pub type MacroExpander = fn(&Literal) -> Result<Option<Literal>, FrontError>;

/// An accepted argument count for a compiler macro.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArityPattern {
    /// The smallest accepted argument count.
    pub minimum: usize,
    /// The largest accepted argument count, or `None` for no upper bound.
    pub maximum: Option<usize>,
}

impl ArityPattern {
    /// Build an arity pattern.
    #[must_use]
    pub const fn new(minimum: usize, maximum: Option<usize>) -> Self {
        Self { minimum, maximum }
    }

    /// Whether `count` arguments are accepted.
    #[must_use]
    pub const fn accepts(&self, count: usize) -> bool {
        count >= self.minimum
            && match self.maximum {
                Some(maximum) => count <= maximum,
                None => true,
            }
    }
}

/// One registered compiler macro.
#[derive(Clone, Debug)]
pub struct CompilerMacro {
    /// The name the compiler macro is registered under.
    pub name: SymbolRef,
    /// The accepted argument counts.
    pub arity: ArityPattern,
    /// The expansion callback.
    pub expander: MacroExpander,
    /// The feature that must be present for the entry to apply.
    pub feature: Option<&'static str>,
}

/// The compiler macros known to the front end.
#[derive(Debug, Default)]
pub struct MacroRegistry {
    entries: Vec<CompilerMacro>,
}

impl MacroRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a compiler macro, replacing any entry with the same name.
    pub fn insert(&mut self, entry: CompilerMacro) {
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|candidate| candidate.name == entry.name)
        {
            *existing = entry;
            return;
        }
        self.entries.push(entry);
    }

    /// Look up a compiler macro by name.
    #[must_use]
    pub fn lookup(&self, name: &SymbolRef) -> Option<&CompilerMacro> {
        self.entries
            .iter()
            .find(|candidate| candidate.name == *name)
    }

    /// The number of registered entries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over the registered entries.
    pub fn iter(&self) -> impl Iterator<Item = &CompilerMacro> {
        self.entries.iter()
    }
}
