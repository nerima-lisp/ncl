//! Heap-resident package namespaces.
use crate::object_access::{get, put};
use crate::widetag;
use crate::{
    ObjectError, Runtime, ThreadContext, make_cons, make_string, make_symbol, rplacd, symbol_name,
};
use ncl_sys::Word;
use std::collections::HashSet;
mod metadata;
crate::word_newtype!(Package);
/// Result of looking up a name in a package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FindStatus {
    Internal,
    External,
    Inherited,
}
pub const LOCK: usize = 0;
pub const GENSYM: usize = 1;
pub const NAME: usize = 2;
pub const NICKNAMES: usize = 3;
pub const USE_LIST: usize = 4;
pub const USED_BY: usize = 5;
pub const INTERNAL: usize = 6;
pub const EXTERNAL: usize = 7;
pub const SHADOWING: usize = 8;
pub const LOCAL_NICKNAMES: usize = 9;
pub const DOCUMENTATION: usize = 10;
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
        DOCUMENTATION,
    ]
}

fn string_text(ctx: &ThreadContext, string: Word) -> Result<String, ObjectError> {
    let length = crate::string_length(ctx, string)?;
    let mut text = String::new();
    for index in 0..length {
        text.push(crate::string_ref(ctx, string, index)?);
    }
    Ok(text)
}

pub(crate) fn symbol_table_entries(
    ctx: &ThreadContext,
    package: Word,
    slot: usize,
) -> Result<Vec<(String, Word)>, ObjectError> {
    let mut entries = get(ctx, package, widetag::PACKAGE, slot)?;
    let mut seen = HashSet::new();
    let mut symbols = Vec::new();
    while entries != Word::NIL {
        if !seen.insert(entries) {
            return Err(ObjectError::Layout);
        }
        let symbol = ncl_sys::read_cons_word(&ctx.thread, entries, 0).ok_or(ObjectError::Layout)?;
        let name = symbol_name(ctx, symbol)?;
        symbols.push((string_text(ctx, name)?, symbol));
        entries = ncl_sys::read_cons_word(&ctx.thread, entries, 1).ok_or(ObjectError::Layout)?;
    }
    Ok(symbols)
}

impl Package {
    /// Allocate an empty package and its internal and external symbol lists.
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
            let object = crate::allocate(ctx, runtime, widetag::PACKAGE, 11)?;
            for (slot, value) in [
                (LOCK, Word::fixnum(0)),
                (GENSYM, Word::fixnum(0)),
                (NAME, *name_word),
                (NICKNAMES, Word::NIL),
                (USE_LIST, Word::NIL),
                (USED_BY, Word::NIL),
                (INTERNAL, Word::NIL),
                (EXTERNAL, Word::NIL),
                (SHADOWING, Word::NIL),
                (LOCAL_NICKNAMES, Word::NIL),
                (DOCUMENTATION, Word::NIL),
            ] {
                put(ctx, object, slot, value)?;
            }
            Ok(Self::from_word(object))
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

    pub fn table_symbol(
        self,
        ctx: &ThreadContext,
        runtime: &Runtime,
        external: bool,
        name: Word,
    ) -> Result<Option<Word>, ObjectError> {
        let key = string_text(ctx, name)?;
        if runtime.package_symbols_indexed(self.0) {
            return runtime.package_symbol(self.0, external, &key);
        }
        let slot = if external { EXTERNAL } else { INTERNAL };
        Ok(symbol_table_entries(ctx, self.0, slot)?
            .into_iter()
            .find_map(|(entry_name, symbol)| (entry_name == key).then_some(symbol)))
    }

    pub fn insert_table_symbol(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        external: bool,
        name: Word,
        symbol: Word,
    ) -> Result<(), ObjectError> {
        let mut roots = [self.0, name, symbol];
        crate::with_rooted_slice(ctx, &mut roots, |ctx, roots| {
            let package = roots.first().copied().ok_or(ObjectError::Layout)?;
            let name = roots.get(1).copied().ok_or(ObjectError::Layout)?;
            let symbol = roots.get(2).copied().ok_or(ObjectError::Layout)?;
            let key = string_text(ctx, name)?;
            let slot = if external { EXTERNAL } else { INTERNAL };
            Self::from_word(package).remove_table_symbol(ctx, runtime, external, name)?;
            let list = get(ctx, package, widetag::PACKAGE, slot)?;
            let list = make_cons(ctx, runtime, symbol, list)?;
            put(ctx, package, slot, list)?;
            runtime.insert_package_symbol(package, external, key, symbol)
        })
    }

    pub fn remove_table_symbol(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        external: bool,
        name: Word,
    ) -> Result<Option<Word>, ObjectError> {
        let mut roots = [self.0, name];
        crate::with_rooted_slice(ctx, &mut roots, |ctx, roots| {
            let package = roots.first().copied().ok_or(ObjectError::Layout)?;
            let name = roots.get(1).copied().ok_or(ObjectError::Layout)?;
            let key = string_text(ctx, name)?;
            let slot = if external { EXTERNAL } else { INTERNAL };
            let indexed_symbol = if runtime.package_symbols_indexed(package) {
                let Some(symbol) = runtime.package_symbol(package, external, &key)? else {
                    return Ok(None);
                };
                Some(symbol)
            } else {
                None
            };
            let mut current = get(ctx, package, widetag::PACKAGE, slot)?;
            let mut previous = Word::NIL;
            let mut seen = HashSet::new();
            while current != Word::NIL {
                if !seen.insert(current) {
                    return Err(ObjectError::Layout);
                }
                let symbol =
                    ncl_sys::read_cons_word(&ctx.thread, current, 0).ok_or(ObjectError::Layout)?;
                let next =
                    ncl_sys::read_cons_word(&ctx.thread, current, 1).ok_or(ObjectError::Layout)?;
                let matches = if let Some(indexed_symbol) = indexed_symbol {
                    symbol == indexed_symbol
                } else {
                    string_text(ctx, symbol_name(ctx, symbol)?)? == key
                };
                if matches {
                    if previous == Word::NIL {
                        put(ctx, package, slot, next)?;
                    } else {
                        rplacd(ctx, previous, next)?;
                    }
                    runtime.remove_package_symbol(package, external, &key)?;
                    return Ok(Some(symbol));
                }
                previous = current;
                current = next;
            }
            runtime.remove_package_symbol(package, external, &key)?;
            Ok(None)
        })
    }

    /// Add a nickname to this package.
    ///
    /// # Errors
    /// Returns an allocation or layout error when the nickname list is malformed.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub(crate) fn register_nickname(
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
        runtime: &Runtime,
        name: Word,
    ) -> Result<Option<(Word, FindStatus)>, ObjectError> {
        if let Some(symbol) = self.table_symbol(ctx, runtime, false, name)? {
            return Ok(Some((symbol, FindStatus::Internal)));
        }
        if let Some(symbol) = self.table_symbol(ctx, runtime, true, name)? {
            return Ok(Some((symbol, FindStatus::External)));
        }
        let mut used = get(ctx, self.0, widetag::PACKAGE, USE_LIST)?;
        while used != Word::NIL {
            let package =
                ncl_sys::read_cons_word(&ctx.thread, used, 0).ok_or(ObjectError::Layout)?;
            let package = Self::from_word(package);
            if let Some(symbol) = package.table_symbol(ctx, runtime, true, name)? {
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
                if let Some(found) =
                    Self::from_word(*package).find_symbol(ctx, runtime, *name_word)?
                {
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
                    let external = runtime.find_package(ctx, "COMMON-LISP") == Some(*package);
                    let table_slot = if external { EXTERNAL } else { INTERNAL };
                    let status = if table_slot == EXTERNAL {
                        FindStatus::External
                    } else {
                        FindStatus::Internal
                    };
                    Self::from_word(*package)
                        .insert_table_symbol(ctx, runtime, external, *name_word, *symbol)?;
                    Ok((*symbol, status))
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
