//! Heap-resident package namespaces.
use crate::hash_table::{HashTable, HashTest, Weakness};
use crate::object_access::{get, put};
use crate::widetag;
use crate::{ObjectError, Runtime, ThreadContext, make_cons, make_string, make_symbol};
use ncl_sys::Word;
mod lists;
mod metadata;
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
    /// Returns an allocation or layout error when package objects cannot be built.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
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
                        (NAME, *name_word),
                        (NICKNAMES, Word::NIL),
                        (USE_LIST, Word::NIL),
                        (USED_BY, Word::NIL),
                        (INTERNAL, *internal),
                        (EXTERNAL, *external),
                        (SHADOWING, Word::NIL),
                        (LOCAL_NICKNAMES, Word::NIL),
                    ] {
                        put(ctx, object, slot, value)?;
                    }
                    Ok(Self::from_word(object))
                })
            })
        })
    }
    /// Return the package name object.
    ///
    /// # Errors
    /// Returns a layout error when the package object is malformed.
    pub fn name(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        get(ctx, self.0, widetag::PACKAGE, NAME)
    }

    /// Return this package's registered nicknames.
    ///
    /// # Errors
    /// Returns a layout error when the package object is malformed.
    pub fn nicknames(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        get(ctx, self.0, widetag::PACKAGE, NICKNAMES)
    }

    /// Return this package's shadowing symbol list.
    ///
    /// # Errors
    /// Returns a layout error when the package object is malformed.
    pub fn shadowing_symbols(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        get(ctx, self.0, widetag::PACKAGE, SHADOWING)
    }

    /// Return the packages used by this package.
    ///
    /// # Errors
    /// Returns an object error when package metadata is malformed.
    pub fn use_list(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        get(ctx, self.0, widetag::PACKAGE, USE_LIST)
    }

    /// Return the packages that use this package.
    ///
    /// # Errors
    /// Returns an object error when package metadata is malformed.
    pub fn used_by_list(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        get(ctx, self.0, widetag::PACKAGE, USED_BY)
    }

    /// Visit every symbol interned in this package.
    ///
    /// # Errors
    /// Returns an object error when package symbol tables are malformed.
    pub fn for_each_symbol<F>(self, ctx: &ThreadContext, mut visit: F) -> Result<(), ObjectError>
    where
        F: FnMut(Word),
    {
        for table in [INTERNAL, EXTERNAL] {
            HashTable::from_word(get(ctx, self.0, widetag::PACKAGE, table)?)
                .for_each_entry(ctx, |_, symbol| visit(symbol))?;
        }
        Ok(())
    }

    /// Add a nickname to this package.
    ///
    /// # Errors
    /// Returns an allocation or layout error when the nickname list is malformed.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn add_nickname(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        nickname: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut nickname = nickname;
            crate::with_root(ctx, &mut nickname, |ctx, nickname| {
                let mut names = get(ctx, *package, widetag::PACKAGE, NICKNAMES)?;
                while names != Word::NIL {
                    if ncl_sys::read_cons_word(&ctx.thread, names, 0) == Some(*nickname) {
                        return Ok(false);
                    }
                    names = ncl_sys::read_cons_word(&ctx.thread, names, 1)
                        .ok_or(ObjectError::Layout)?;
                }
                let names = make_cons(
                    ctx,
                    runtime,
                    *nickname,
                    get(ctx, *package, widetag::PACKAGE, NICKNAMES)?,
                )?;
                put(ctx, *package, NICKNAMES, names)?;
                Ok(true)
            })
        })
    }
    /// Find an accessible symbol in this package.
    ///
    /// # Errors
    /// Returns a layout error when an accessible symbol or package list is malformed.
    pub fn find_symbol(
        self,
        ctx: &mut ThreadContext,
        name: Word,
    ) -> Result<Option<(Word, FindStatus)>, ObjectError> {
        let internal = HashTable::from_word(get(ctx, self.0, widetag::PACKAGE, INTERNAL)?);
        if let Some(symbol) = internal.get(ctx, name)? {
            return Ok(Some((symbol, FindStatus::Internal)));
        }
        let external = HashTable::from_word(get(ctx, self.0, widetag::PACKAGE, EXTERNAL)?);
        if let Some(symbol) = external.get(ctx, name)? {
            return Ok(Some((symbol, FindStatus::External)));
        }
        let mut used = get(ctx, self.0, widetag::PACKAGE, USE_LIST)?;
        while used != Word::NIL {
            let package =
                ncl_sys::read_cons_word(&ctx.thread, used, 0).ok_or(ObjectError::Layout)?;
            let package = Self::from_word(package);
            let external = HashTable::from_word(get(ctx, package.0, widetag::PACKAGE, EXTERNAL)?);
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
    /// Returns an allocation or layout error when the symbol tables are malformed.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn intern(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: &str,
    ) -> Result<(Word, FindStatus), ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
            crate::with_root(ctx, &mut name_word, |ctx, name_word| {
                if let Some(found) = Self::from_word(*package).find_symbol(ctx, *name_word)? {
                    return Ok(found);
                }
                let mut symbol = make_symbol(ctx, runtime, *name_word)?;
                crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                    put(
                        ctx,
                        *symbol,
                        crate::layout::symbol_offset::PACKAGE,
                        *package,
                    )?;
                    let table =
                        HashTable::from_word(get(ctx, *package, widetag::PACKAGE, INTERNAL)?);
                    table.insert(ctx, runtime, *name_word, *symbol)?;
                    Ok((*symbol, FindStatus::Internal))
                })
            })
        })
    }
    /// Export an internal symbol by moving it to the external table.
    ///
    /// # Errors
    /// Returns an allocation or layout error when the symbol tables are malformed.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn export(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            crate::with_root(ctx, &mut name, |ctx, name| {
                let internal =
                    HashTable::from_word(get(ctx, *package, widetag::PACKAGE, INTERNAL)?);
                let mut symbol = if let Some(symbol) = internal.remove(ctx, runtime, *name)? {
                    symbol
                } else {
                    let external =
                        HashTable::from_word(get(ctx, *package, widetag::PACKAGE, EXTERNAL)?);
                    if external.get(ctx, *name)?.is_some() {
                        return Ok(true);
                    }
                    let Some((symbol, FindStatus::Inherited)) =
                        Self::from_word(*package).find_symbol(ctx, *name)?
                    else {
                        return Ok(false);
                    };
                    Self::from_word(*package).import(ctx, runtime, *name, symbol)?;
                    HashTable::from_word(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                        .remove(ctx, runtime, *name)?
                        .ok_or(ObjectError::Layout)?
                };
                crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                    let external =
                        HashTable::from_word(get(ctx, *package, widetag::PACKAGE, EXTERNAL)?);
                    external.insert(ctx, runtime, *name, *symbol)
                })?;
                Ok(true)
            })
        })
    }
    /// Remove a symbol from the external table and return it to internal visibility.
    ///
    /// # Errors
    /// Returns an allocation or layout error when the symbol tables are malformed.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn unexport(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            crate::with_root(ctx, &mut name, |ctx, name| {
                let external =
                    HashTable::from_word(get(ctx, *package, widetag::PACKAGE, EXTERNAL)?);
                let Some(mut symbol) = external.remove(ctx, runtime, *name)? else {
                    return Ok(false);
                };
                crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                    let internal =
                        HashTable::from_word(get(ctx, *package, widetag::PACKAGE, INTERNAL)?);
                    internal.insert(ctx, runtime, *name, *symbol)
                })?;
                Ok(true)
            })
        })
    }
    /// Import a symbol under a string name.
    ///
    /// # Errors
    /// Returns an allocation or layout error when the symbol tables are malformed.
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
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            crate::with_root(ctx, &mut name, |ctx, name| {
                let mut symbol = symbol;
                crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                    let home = get(
                        ctx,
                        *symbol,
                        widetag::SYMBOL,
                        crate::layout::symbol_offset::PACKAGE,
                    )?;
                    if let Some((existing, status)) =
                        Self::from_word(*package).find_symbol(ctx, *name)?
                    {
                        if existing == *symbol {
                            if status != FindStatus::Inherited {
                                return Ok(());
                            }
                        } else {
                            return Err(ObjectError::PackageConflict);
                        }
                    }
                    if home == Word::NIL {
                        put(
                            ctx,
                            *symbol,
                            crate::layout::symbol_offset::PACKAGE,
                            *package,
                        )?;
                    }
                    HashTable::from_word(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                        .insert(ctx, runtime, *name, *symbol)
                })
            })
        })
    }
    /// Generate an uninterned symbol.
    ///
    /// # Errors
    /// Returns an allocation or layout error when the gensym counter or symbol cannot be built.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn gensym(self, ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            let number = get(ctx, *package, widetag::PACKAGE, GENSYM)?
                .as_fixnum()
                .ok_or(ObjectError::Layout)?;
            put(ctx, *package, GENSYM, Word::fixnum(number + 1))?;
            let name = format!("G{number}");
            let name = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
            make_symbol(ctx, runtime, name)
        })
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
    fn unintern_does_not_remove_external_when_internal_matches() {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        let mut package = Package::new(&mut ctx, &runtime, "DUPLICATES")
            .unwrap_or_else(|error| panic!("package: {error:?}"))
            .as_word();
        let package_token = crate::push_root(&mut ctx, &mut package);
        let name = make_string(&mut ctx, &runtime, &['D', 'U', 'P'])
            .unwrap_or_else(|error| panic!("name: {error:?}"));
        let mut name = name;
        let name_token = crate::push_root(&mut ctx, &mut name);
        let internal = make_symbol(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("internal: {error:?}"));
        let mut internal = internal;
        let internal_token = crate::push_root(&mut ctx, &mut internal);
        let external = make_symbol(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("external: {error:?}"));
        let mut external = external;
        let external_token = crate::push_root(&mut ctx, &mut external);
        put(
            &mut ctx,
            internal,
            crate::layout::symbol_offset::PACKAGE,
            package,
        )
        .unwrap_or_else(|error| panic!("internal home: {error:?}"));
        put(
            &mut ctx,
            external,
            crate::layout::symbol_offset::PACKAGE,
            package,
        )
        .unwrap_or_else(|error| panic!("external home: {error:?}"));
        HashTable::from_word(
            get(&ctx, package, widetag::PACKAGE, INTERNAL)
                .unwrap_or_else(|error| panic!("internal table: {error:?}")),
        )
        .insert(&mut ctx, &runtime, name, internal)
        .unwrap_or_else(|error| panic!("internal insert: {error:?}"));
        HashTable::from_word(
            get(&ctx, package, widetag::PACKAGE, EXTERNAL)
                .unwrap_or_else(|error| panic!("external table: {error:?}")),
        )
        .insert(&mut ctx, &runtime, name, external)
        .unwrap_or_else(|error| panic!("external insert: {error:?}"));

        assert!(
            Package::from_word(package)
                .unintern(&mut ctx, &runtime, name)
                .unwrap_or_else(|error| panic!("unintern: {error:?}"))
        );
        assert_eq!(
            HashTable::from_word(
                get(&ctx, package, widetag::PACKAGE, EXTERNAL)
                    .unwrap_or_else(|error| panic!("external table: {error:?}")),
            )
            .get(&mut ctx, name),
            Ok(Some(external))
        );
        assert_eq!(
            HashTable::from_word(
                get(&ctx, package, widetag::PACKAGE, INTERNAL)
                    .unwrap_or_else(|error| panic!("internal table: {error:?}")),
            )
            .get(&mut ctx, name),
            Ok(None)
        );
        assert!(crate::pop_root(&mut ctx, external_token));
        assert!(crate::pop_root(&mut ctx, internal_token));
        assert!(crate::pop_root(&mut ctx, name_token));
        assert!(crate::pop_root(&mut ctx, package_token));
    }
}
