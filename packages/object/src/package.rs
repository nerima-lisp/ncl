//! Symbols and package namespace invariants.

use crate::{ObjectError, Runtime, ThreadContext, make_symbol};
use ncl_sys::Word;
use std::collections::{HashMap, HashSet};

/// Result of looking up a name in a package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FindStatus {
    Internal,
    External,
    Inherited,
}

/// A package namespace with explicit exports, shadows, and used packages.
#[derive(Debug)]
pub struct Package {
    name: String,
    symbols: HashMap<String, Word>,
    exports: HashSet<String>,
    shadows: HashSet<String>,
    used: Vec<String>,
    gensym: u64,
}

impl Package {
    /// Create an empty package with a name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            symbols: HashMap::new(),
            exports: HashSet::new(),
            shadows: HashSet::new(),
            used: Vec::new(),
            gensym: 0,
        }
    }
    /// Return the package name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Find an accessible symbol and its status.
    #[must_use]
    pub fn find_symbol(
        &self,
        name: &str,
        packages: &HashMap<String, Package>,
    ) -> Option<(Word, FindStatus)> {
        if let Some(symbol) = self.symbols.get(name) {
            return Some((
                *symbol,
                if self.exports.contains(name) {
                    FindStatus::External
                } else {
                    FindStatus::Internal
                },
            ));
        }
        if self.shadows.contains(name) {
            return None;
        }
        self.used.iter().find_map(|used| {
            packages
                .get(used)?
                .symbols
                .get(name)
                .map(|symbol| (*symbol, FindStatus::Inherited))
        })
    }
    /// Intern a name, preserving the package's one-symbol-per-name invariant.
    pub fn intern(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: &str,
    ) -> Result<(Word, FindStatus), ObjectError> {
        if let Some(found) = self.symbols.get(name) {
            return Ok((
                *found,
                if self.exports.contains(name) {
                    FindStatus::External
                } else {
                    FindStatus::Internal
                },
            ));
        }
        let symbol = make_symbol(ctx, runtime, Word::NIL)?;
        self.symbols.insert(name.to_owned(), symbol);
        Ok((symbol, FindStatus::Internal))
    }
    /// Remove a symbol from this package.
    pub fn unintern(&mut self, name: &str) -> bool {
        self.exports.remove(name);
        self.shadows.remove(name);
        self.symbols.remove(name).is_some()
    }
    /// Mark an internal symbol external.
    pub fn export(&mut self, name: &str) -> bool {
        self.symbols.contains_key(name) && self.exports.insert(name.to_owned())
    }
    /// Import a symbol under its printed name.
    pub fn import(&mut self, name: impl Into<String>, symbol: Word) -> bool {
        self.symbols.insert(name.into(), symbol).is_none()
    }
    /// Reserve a name against inherited symbols.
    pub fn shadow(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.shadows.insert(name.clone());
        self.symbols.entry(name).or_insert(Word::NIL);
    }
    /// Add another package to the use list.
    pub fn use_package(&mut self, name: impl Into<String>) -> bool {
        let name = name.into();
        if self.used.contains(&name) {
            false
        } else {
            self.used.push(name);
            true
        }
    }
    /// Generate a fresh uninterned symbol placeholder.
    pub fn gensym(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
    ) -> Result<Word, ObjectError> {
        self.gensym += 1;
        make_symbol(ctx, runtime, Word::NIL)
    }
}

/// Canonical static NIL value.
#[must_use]
pub const fn nil() -> Word {
    Word::NIL
}
/// Canonical static true value.
#[must_use]
pub const fn truth() -> Word {
    Word::TRUE
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_intern_is_unique_and_unintern_reopens_name() {
        let runtime = Runtime::new();
        let mut ctx = ThreadContext::new();
        assert!(ctx.register(&runtime).is_ok());
        let mut package = Package::new("NCL");
        let first = package.intern(&mut ctx, &runtime, "X").map(|pair| pair.0);
        let second = package.intern(&mut ctx, &runtime, "X").map(|pair| pair.0);
        assert_eq!(first, second);
        assert!(package.unintern("X"));
        assert!(package.intern(&mut ctx, &runtime, "X").is_ok());
    }
}
