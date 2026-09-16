//! Heap-resident package namespaces.
use crate::hash_table::{HashTable, HashTest, Weakness};
use crate::object_access::{get, put};
use crate::widetag;
use crate::{ObjectError, Runtime, ThreadContext, make_cons, make_string, make_symbol, rplacd};
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
                    Ok(object.into())
                })
            })
        })
    }
    /// Return the package name object.
    ///
    /// # Errors
    pub fn name(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        get(ctx, self.0, widetag::PACKAGE, NAME)
    }
    /// Return this package's shadowing symbol list.
    ///
    /// # Errors
    /// Returns a layout error when the package object is malformed.
    pub fn shadowing_symbols(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        get(ctx, self.0, widetag::PACKAGE, SHADOWING)
    }
    /// Add a nickname to this package.
    ///
    /// # Errors
    pub fn add_nickname(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        nickname: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
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
    pub fn intern(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: &str,
    ) -> Result<(Word, FindStatus), ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            let mut name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
            crate::with_root(ctx, &mut name_word, |ctx, name_word| {
                if let Some(found) = Self::from(*package).find_symbol(ctx, *name_word)? {
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
                    let table = HashTable::from(get(ctx, *package, widetag::PACKAGE, INTERNAL)?);
                    table.insert(ctx, runtime, *name_word, *symbol)?;
                    Ok((*symbol, FindStatus::Internal))
                })
            })
        })
    }
    /// Export an internal symbol by moving it to the external table.
    ///
    /// # Errors
    pub fn export(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            let mut name = name;
            crate::with_root(ctx, &mut name, |ctx, name| {
                let internal = HashTable::from(get(ctx, *package, widetag::PACKAGE, INTERNAL)?);
                let mut symbol = if let Some(symbol) = internal.remove(ctx, runtime, *name)? {
                    symbol
                } else {
                    let external = HashTable::from(get(ctx, *package, widetag::PACKAGE, EXTERNAL)?);
                    if external.get(ctx, *name)?.is_some() {
                        return Ok(true);
                    }
                    let Some((symbol, FindStatus::Inherited)) =
                        Self::from(*package).find_symbol(ctx, *name)?
                    else {
                        return Ok(false);
                    };
                    Self::from(*package).import(ctx, runtime, *name, symbol)?;
                    HashTable::from(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                        .remove(ctx, runtime, *name)?
                        .ok_or(ObjectError::Layout)?
                };
                crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                    let external = HashTable::from(get(ctx, *package, widetag::PACKAGE, EXTERNAL)?);
                    external.insert(ctx, runtime, *name, *symbol)
                })?;
                Ok(true)
            })
        })
    }
    /// Remove a symbol from the external table and return it to internal visibility.
    ///
    /// # Errors
    pub fn unexport(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            let mut name = name;
            crate::with_root(ctx, &mut name, |ctx, name| {
                let external = HashTable::from(get(ctx, *package, widetag::PACKAGE, EXTERNAL)?);
                let Some(mut symbol) = external.remove(ctx, runtime, *name)? else {
                    return Ok(false);
                };
                crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                    let internal = HashTable::from(get(ctx, *package, widetag::PACKAGE, INTERNAL)?);
                    internal.insert(ctx, runtime, *name, *symbol)
                })?;
                Ok(true)
            })
        })
    }
    /// Import a symbol under a string name.
    ///
    /// # Errors
    pub fn import(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
        symbol: Word,
    ) -> Result<(), ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
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
                        Self::from(*package).find_symbol(ctx, *name)?
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
                    HashTable::from(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                        .insert(ctx, runtime, *name, *symbol)
                })
            })
        })
    }
    /// Add another package to this package's use list.
    ///
    /// # Errors
    pub fn use_package(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        package: Word,
    ) -> Result<bool, ObjectError> {
        let mut package_self = self.0;
        crate::with_root(ctx, &mut package_self, |ctx, package_self| {
            let mut package = package;
            crate::with_root(ctx, &mut package, |ctx, package| {
                let mut list = get(ctx, *package_self, widetag::PACKAGE, USE_LIST)?;
                while list != Word::NIL {
                    if ncl_sys::read_cons_word(&ctx.thread, list, 0) == Some(*package) {
                        return Ok(false);
                    }
                    list =
                        ncl_sys::read_cons_word(&ctx.thread, list, 1).ok_or(ObjectError::Layout)?;
                }
                let list = make_cons(
                    ctx,
                    runtime,
                    *package,
                    get(ctx, *package_self, widetag::PACKAGE, USE_LIST)?,
                )?;
                put(ctx, *package_self, USE_LIST, list)?;
                let mut used_by = get(ctx, *package, widetag::PACKAGE, USED_BY)?;
                while used_by != Word::NIL {
                    if ncl_sys::read_cons_word(&ctx.thread, used_by, 0) == Some(*package_self) {
                        return Ok(true);
                    }
                    used_by = ncl_sys::read_cons_word(&ctx.thread, used_by, 1)
                        .ok_or(ObjectError::Layout)?;
                }
                let used_by = make_cons(
                    ctx,
                    runtime,
                    *package_self,
                    get(ctx, *package, widetag::PACKAGE, USED_BY)?,
                )?;
                put(ctx, *package, USED_BY, used_by)?;
                Ok(true)
            })
        })
    }
    /// Remove another package from this package's use list.
    ///
    /// # Errors
    pub fn unuse_package(
        self,
        ctx: &mut ThreadContext,
        package: Word,
    ) -> Result<bool, ObjectError> {
        let mut package_self = self.0;
        crate::with_root(ctx, &mut package_self, |ctx, package_self| {
            let mut package = package;
            crate::with_root(ctx, &mut package, |ctx, package| {
                let removed = remove_from_list(ctx, *package_self, USE_LIST, *package)?;
                if !removed {
                    return Ok(false);
                }
                remove_from_list(ctx, *package, USED_BY, *package_self)?;
                Ok(true)
            })
        })
    }
    /// Remove a symbol from internal or external visibility.
    ///
    /// # Errors
    pub fn unintern(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            let mut name = name;
            crate::with_root(ctx, &mut name, |ctx, name| {
                let symbol = match HashTable::from(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                    .remove(ctx, runtime, *name)?
                {
                    Some(symbol) => Some(symbol),
                    None => HashTable::from(get(ctx, *package, widetag::PACKAGE, EXTERNAL)?)
                        .remove(ctx, runtime, *name)?,
                };
                let Some(mut symbol) = symbol else {
                    return Ok(false);
                };
                crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                    if get(
                        ctx,
                        *symbol,
                        widetag::SYMBOL,
                        crate::layout::symbol_offset::PACKAGE,
                    )? == *package
                    {
                        put(
                            ctx,
                            *symbol,
                            crate::layout::symbol_offset::PACKAGE,
                            Word::NIL,
                        )?;
                    }
                    let mut current = get(ctx, *package, widetag::PACKAGE, SHADOWING)?;
                    let mut previous = Word::NIL;
                    while current != Word::NIL {
                        let next = ncl_sys::read_cons_word(&ctx.thread, current, 1)
                            .ok_or(ObjectError::Layout)?;
                        if ncl_sys::read_cons_word(&ctx.thread, current, 0) == Some(*symbol) {
                            if previous == Word::NIL {
                                put(ctx, *package, SHADOWING, next)?;
                            } else {
                                rplacd(ctx, previous, next)?;
                            }
                            break;
                        }
                        previous = current;
                        current = next;
                    }
                    Ok(true)
                })
            })
        })
    }
    /// Add a name to the package's shadowing list.
    ///
    /// # Errors
    pub fn shadow(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<(), ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            let mut name = name;
            crate::with_root(ctx, &mut name, |ctx, name| {
                let symbol =
                    if let Some((symbol, _)) = Self::from(*package).find_symbol(ctx, *name)? {
                        symbol
                    } else {
                        let mut symbol = make_symbol(ctx, runtime, *name)?;
                        crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                            put(
                                ctx,
                                *symbol,
                                crate::layout::symbol_offset::PACKAGE,
                                *package,
                            )?;
                            HashTable::from(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                                .insert(ctx, runtime, *name, *symbol)?;
                            Ok(*symbol)
                        })?
                    };
                let mut list = get(ctx, *package, widetag::PACKAGE, SHADOWING)?;
                while list != Word::NIL {
                    if ncl_sys::read_cons_word(&ctx.thread, list, 0) == Some(symbol) {
                        return Ok(());
                    }
                    list =
                        ncl_sys::read_cons_word(&ctx.thread, list, 1).ok_or(ObjectError::Layout)?;
                }
                let list = make_cons(
                    ctx,
                    runtime,
                    symbol,
                    get(ctx, *package, widetag::PACKAGE, SHADOWING)?,
                )?;
                put(ctx, *package, SHADOWING, list)
            })
        })
    }
    /// Generate an uninterned symbol.
    ///
    /// # Errors
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
fn remove_from_list(
    ctx: &mut ThreadContext,
    object: Word,
    slot: usize,
    target: Word,
) -> Result<bool, ObjectError> {
    let mut current = get(ctx, object, widetag::PACKAGE, slot)?;
    let mut previous = Word::NIL;
    while current != Word::NIL {
        let next = ncl_sys::read_cons_word(&ctx.thread, current, 1).ok_or(ObjectError::Layout)?;
        if ncl_sys::read_cons_word(&ctx.thread, current, 0) == Some(target) {
            if previous == Word::NIL {
                put(ctx, object, slot, next)?;
            } else {
                rplacd(ctx, previous, next)?;
            }
            return Ok(true);
        }
        previous = current;
        current = next;
    }
    Ok(false)
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
        let package = Package::new(&mut ctx, &runtime, "DUPLICATES")
            .unwrap_or_else(|error| panic!("package: {error:?}"));
        let name = make_string(&mut ctx, &runtime, &['D', 'U', 'P'])
            .unwrap_or_else(|error| panic!("name: {error:?}"));
        let internal = make_symbol(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("internal: {error:?}"));
        let external = make_symbol(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("external: {error:?}"));
        put(
            &mut ctx,
            internal,
            crate::layout::symbol_offset::PACKAGE,
            package.as_word(),
        )
        .unwrap_or_else(|error| panic!("internal home: {error:?}"));
        put(
            &mut ctx,
            external,
            crate::layout::symbol_offset::PACKAGE,
            package.as_word(),
        )
        .unwrap_or_else(|error| panic!("external home: {error:?}"));
        HashTable::from(get(&ctx, package.as_word(), widetag::PACKAGE, INTERNAL).unwrap())
            .insert(&mut ctx, &runtime, name, internal)
            .unwrap_or_else(|error| panic!("internal insert: {error:?}"));
        HashTable::from(get(&ctx, package.as_word(), widetag::PACKAGE, EXTERNAL).unwrap())
            .insert(&mut ctx, &runtime, name, external)
            .unwrap_or_else(|error| panic!("external insert: {error:?}"));

        assert!(
            package
                .unintern(&mut ctx, &runtime, name)
                .unwrap_or_else(|error| panic!("unintern: {error:?}"))
        );
        assert_eq!(
            HashTable::from(get(&ctx, package.as_word(), widetag::PACKAGE, EXTERNAL).unwrap())
                .get(&mut ctx, name),
            Ok(Some(external))
        );
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
