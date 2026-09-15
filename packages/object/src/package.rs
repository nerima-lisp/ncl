//! Heap-resident package namespaces.

use crate::hash_table::{HashTable, HashTest, Weakness};
use crate::object_access::{get, put};
use crate::widetag;
use crate::{ObjectError, Runtime, ThreadContext, make_simple_vector, make_string, make_symbol};
use crate::{string_length, string_ref};
use ncl_sys::Word;

crate::word_newtype!(Package);

/// Result of looking up a name in a package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FindStatus {
    Internal,
    External,
    Inherited,
}

const NAME: usize = 0;
const NICKNAMES: usize = 1;
const USE_LIST: usize = 2;
const USED_BY: usize = 3;
const INTERNAL: usize = 4;
const EXTERNAL: usize = 5;
const SHADOWING: usize = 6;
const LOCAL_NICKNAMES: usize = 7;
const LOCK: usize = 8;
const GENSYM: usize = 9;

impl Package {
    /// Allocate an empty package and its internal and external symbol tables.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    pub fn new(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: &str,
    ) -> Result<Self, ObjectError> {
        let name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
        let internal = HashTable::new(ctx, runtime, HashTest::Equal, Weakness::None)?.as_word();
        let external = HashTable::new(ctx, runtime, HashTest::Equal, Weakness::None)?.as_word();
        let object = crate::allocate(ctx, runtime, widetag::PACKAGE, 10)?;
        for (slot, value) in [
            (NAME, name_word),
            (NICKNAMES, Word::NIL),
            (USE_LIST, Word::NIL),
            (USED_BY, Word::NIL),
            (INTERNAL, internal),
            (EXTERNAL, external),
            (SHADOWING, Word::NIL),
            (LOCAL_NICKNAMES, Word::NIL),
            (LOCK, Word::fixnum(0)),
            (GENSYM, Word::fixnum(0)),
        ] {
            put(ctx, object, slot, value)?;
        }
        Ok(object.into())
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
        Ok(external
            .get(ctx, name)?
            .map(|symbol| (symbol, FindStatus::External)))
    }
    /// Intern a symbol by name, returning its symbol and status.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    pub fn intern(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: &str,
    ) -> Result<(Word, FindStatus), ObjectError> {
        let name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
        let internal = HashTable::from(get(ctx, self.0, widetag::PACKAGE, INTERNAL)?);
        for (key, symbol) in internal.map_entries(ctx)? {
            if strings_equal(ctx, key, name_word)? {
                return Ok((symbol, FindStatus::Internal));
            }
        }
        if let Some(found) = self.find_symbol(ctx, name_word)? {
            return Ok(found);
        }
        let symbol = make_symbol(ctx, runtime, name_word)?;
        put(ctx, symbol, crate::layout::symbol_offset::PACKAGE, self.0)?;
        let table = HashTable::from(get(ctx, self.0, widetag::PACKAGE, INTERNAL)?);
        table.insert(ctx, runtime, name_word, symbol)?;
        Ok((symbol, FindStatus::Internal))
    }
    /// Export an internal symbol by moving it to the external table.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
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
        HashTable::from(get(ctx, self.0, widetag::PACKAGE, EXTERNAL)?)
            .insert(ctx, runtime, name, symbol)?;
        Ok(true)
    }
    /// Add a name to the package's shadowing list.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    pub fn shadow(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<(), ObjectError> {
        let list = get(ctx, self.0, widetag::PACKAGE, SHADOWING)?;
        let vector = make_simple_vector(ctx, runtime, &[name, list])?;
        put(ctx, self.0, SHADOWING, vector)
    }
    /// Generate an uninterned symbol.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    pub fn gensym(self, ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
        let number = get(ctx, self.0, widetag::PACKAGE, GENSYM)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?;
        put(ctx, self.0, GENSYM, Word::fixnum(number + 1))?;
        make_symbol(ctx, runtime, Word::NIL)
    }
}

fn strings_equal(ctx: &ThreadContext, left: Word, right: Word) -> Result<bool, ObjectError> {
    if ncl_sys::object_widetag(&ctx.thread, left) != Some(widetag::STRING)
        || ncl_sys::object_widetag(&ctx.thread, right) != Some(widetag::STRING)
    {
        return Ok(left == right);
    }
    let length = string_length(ctx, left)?;
    if length != string_length(ctx, right)? {
        return Ok(false);
    }
    for index in 0..length {
        if string_ref(ctx, left, index)? != string_ref(ctx, right, index)? {
            return Ok(false);
        }
    }
    Ok(true)
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
