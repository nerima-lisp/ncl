//! Heap-resident package namespaces.

use crate::hash_table::{HashTable, HashTest, Weakness};
use crate::object_access::{get, put};
use crate::widetag;
use crate::{ObjectError, Runtime, ThreadContext, make_cons, make_string, make_symbol};
use ncl_sys::Word;

crate::word_newtype!(Package);

/// Result of looking up a name in a package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FindStatus {
    Internal,
    External,
    Inherited,
}

pub(crate) const LOCK: usize = 0;
pub(crate) const GENSYM: usize = 1;
pub(crate) const NAME: usize = 2;
pub(crate) const NICKNAMES: usize = 3;
pub(crate) const USE_LIST: usize = 4;
pub(crate) const USED_BY: usize = 5;
pub(crate) const INTERNAL: usize = 6;
pub(crate) const EXTERNAL: usize = 7;
pub(crate) const SHADOWING: usize = 8;
pub(crate) const LOCAL_NICKNAMES: usize = 9;

pub(crate) fn reference_words() -> Vec<usize> {
    vec![
        NAME,
        NICKNAMES,
        USE_LIST,
        USED_BY,
        INTERNAL,
        EXTERNAL,
        SHADOWING,
        LOCAL_NICKNAMES,
    ]
}

impl Package {
    /// Allocate an empty package and its internal and external symbol tables.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    ///
    pub fn new(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: &str,
    ) -> Result<Self, ObjectError> {
        let mut name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
        crate::with_root(ctx, &mut name_word, |ctx, name_word| {
            let mut internal =
                HashTable::new(ctx, runtime, HashTest::Equal, Weakness::None)?.as_word();
            crate::with_root(ctx, &mut internal, |ctx, internal| {
                let mut external =
                    HashTable::new(ctx, runtime, HashTest::Equal, Weakness::None)?.as_word();
                crate::with_root(ctx, &mut external, |ctx, external| {
                    let object = crate::allocate(ctx, runtime, widetag::PACKAGE, 10)?;
                    for (slot, value) in [
                        (LOCK, Word::fixnum(0)),
                        (GENSYM, Word::fixnum(0)),
                        (NAME, name_word),
                        (NICKNAMES, Word::NIL),
                        (USE_LIST, Word::NIL),
                        (USED_BY, Word::NIL),
                        (INTERNAL, internal),
                        (EXTERNAL, external),
                        (SHADOWING, Word::NIL),
                        (LOCAL_NICKNAMES, Word::NIL),
                    ] {
                        put(ctx, object, slot, value)?;
                    }
                    Ok(object.into())
                })
            })
        })
    }
    /// Return the package name object.
    ///
    /// # Errors
    /// Returns an error for an invalid heap layout.
    pub fn name(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        get(ctx, self.0, widetag::PACKAGE, NAME)
    }
    /// Find an accessible symbol in this package.
    ///
    /// # Errors
    /// Returns an error for an invalid heap layout.
    pub fn find_symbol(
        self,
        ctx: &mut ThreadContext,
        name: Word,
    ) -> Result<Option<(Word, FindStatus)>, ObjectError> {
        let internal = HashTable::from(get(ctx, self.0, widetag::PACKAGE, INTERNAL)?);
        if let Some(symbol) = internal.get(ctx, name)? {
            return Ok(Some((symbol, FindStatus::Internal)));
        }
        let external = HashTable::from(get(ctx, self.0, widetag::PACKAGE, EXTERNAL)?);
        if let Some(symbol) = external.get(ctx, name)? {
            return Ok(Some((symbol, FindStatus::External)));
        }
        let mut used = get(ctx, self.0, widetag::PACKAGE, USE_LIST)?;
        while used != Word::NIL {
            let package =
                ncl_sys::read_cons_word(&ctx.thread, used, 0).ok_or(ObjectError::Layout)?;
            let package = Self::from(package);
            let external = HashTable::from(get(ctx, package.0, widetag::PACKAGE, EXTERNAL)?);
            if let Some(symbol) = external.get(ctx, name)? {
                return Ok(Some((symbol, FindStatus::Inherited)));
            }
            used = ncl_sys::read_cons_word(&ctx.thread, used, 1).ok_or(ObjectError::Layout)?;
        }
        Ok(None)
    }
    /// Intern a symbol by name, returning its symbol and status.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn intern(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: &str,
    ) -> Result<(Word, FindStatus), ObjectError> {
        let mut name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
        crate::with_root(ctx, &mut name_word, |ctx, name_word| {
            if let Some(found) = self.find_symbol(ctx, name_word)? {
                return Ok(found);
            }
            let mut package = self.0;
            crate::with_root(ctx, &mut package, |ctx, package| {
                let mut symbol = make_symbol(ctx, runtime, name_word)?;
                crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                    put(ctx, symbol, crate::layout::symbol_offset::PACKAGE, package)?;
                    let table = HashTable::from(get(ctx, package, widetag::PACKAGE, INTERNAL)?);
                    table.insert(ctx, runtime, name_word, symbol)?;
                    Ok((symbol, FindStatus::Internal))
                })
            })
        })
    }
    /// Export an internal symbol by moving it to the external table.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn export(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let internal = HashTable::from(get(ctx, self.0, widetag::PACKAGE, INTERNAL)?);
        let Some(symbol) = internal.remove(ctx, runtime, name)? else {
            return Ok(false);
        };
        let external = HashTable::from(get(ctx, self.0, widetag::PACKAGE, EXTERNAL)?);
        let mut name = name;
        crate::with_root(ctx, &mut name, |ctx, name| {
            let mut symbol = symbol;
            crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                external.insert(ctx, runtime, name, symbol)
            })
        })?;
        Ok(true)
    }
    /// Remove a symbol from the external table and return it to internal visibility.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn unexport(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let external = HashTable::from(get(ctx, self.0, widetag::PACKAGE, EXTERNAL)?);
        let Some(symbol) = external.remove(ctx, runtime, name)? else {
            return Ok(false);
        };
        let internal = HashTable::from(get(ctx, self.0, widetag::PACKAGE, INTERNAL)?);
        let mut name = name;
        crate::with_root(ctx, &mut name, |ctx, name| {
            let mut symbol = symbol;
            crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                internal.insert(ctx, runtime, name, symbol)
            })
        })?;
        Ok(true)
    }
    /// Import a symbol under a string name.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn import(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
        symbol: Word,
    ) -> Result<(), ObjectError> {
        put(ctx, symbol, crate::layout::symbol_offset::PACKAGE, self.0)?;
        HashTable::from(get(ctx, self.0, widetag::PACKAGE, INTERNAL)?)
            .insert(ctx, runtime, name, symbol)
    }
    /// Add another package to this package's use list.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn use_package(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        package: Word,
    ) -> Result<bool, ObjectError> {
        let mut list = get(ctx, self.0, widetag::PACKAGE, USE_LIST)?;
        while list != Word::NIL {
            if ncl_sys::read_cons_word(&ctx.thread, list, 0) == Some(package) {
                return Ok(false);
            }
            list = ncl_sys::read_cons_word(&ctx.thread, list, 1).ok_or(ObjectError::Layout)?;
        }
        let list = make_cons(
            ctx,
            runtime,
            package,
            get(ctx, self.0, widetag::PACKAGE, USE_LIST)?,
        )?;
        put(ctx, self.0, USE_LIST, list)?;
        Ok(true)
    }
    /// Remove a symbol from internal or external visibility.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn unintern(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let internal = HashTable::from(get(ctx, self.0, widetag::PACKAGE, INTERNAL)?);
        if internal.remove(ctx, runtime, name)?.is_some() {
            return Ok(true);
        }
        Ok(
            HashTable::from(get(ctx, self.0, widetag::PACKAGE, EXTERNAL)?)
                .remove(ctx, runtime, name)?
                .is_some(),
        )
    }
    /// Add a name to the package's shadowing list.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn shadow(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<(), ObjectError> {
        let list = get(ctx, self.0, widetag::PACKAGE, SHADOWING)?;
        let list = make_cons(ctx, runtime, name, list)?;
        put(ctx, self.0, SHADOWING, list)
    }
    /// Generate an uninterned symbol.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn gensym(self, ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
        let number = get(ctx, self.0, widetag::PACKAGE, GENSYM)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?;
        put(ctx, self.0, GENSYM, Word::fixnum(number + 1))?;
        let name = format!("G{number}");
        let name = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
        make_symbol(ctx, runtime, name)
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
