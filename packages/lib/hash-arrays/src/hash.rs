//! Typed hash-table domain operations and builtin-boundary views.

use ncl_object::hash_table::{HashTable, HashTest, Weakness, sxhash};
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, LispError, MultipleValues, ObjectError, ObjectType, Parameter,
    ParameterType, Runtime, ThreadContext, Word, classify_object, string_length, string_ref,
    symbol_name,
};

#[path = "builtins.rs"]
mod builtins;
pub use builtins::register;

/// An opaque Lisp value accepted as a hash-table key or value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LispValue(Word);

impl LispValue {
    /// Wrap a value at the builtin boundary.
    #[must_use]
    pub const fn from_word(word: Word) -> Self {
        Self(word)
    }

    /// Return the ABI representation at the builtin boundary.
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.0
    }
}

/// A checked view of a heap hash table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HashTableView(HashTable);

impl HashTableView {
    /// Validate a Lisp value as a hash table.
    pub fn try_from_word(ctx: &ThreadContext, word: Word) -> Result<Self, LispError> {
        match classify_object(ctx, word) {
            ncl_object::ObjectRef::HashTable(table) => Ok(Self(HashTable::from_word(table))),
            _ => Err(LispError::TypeError {
                datum: word,
                expected: ObjectType::HashTable,
            }),
        }
    }

    /// Return the ABI representation at the builtin boundary.
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.0.as_word()
    }
}

/// The typed options used by `MAKE-HASH-TABLE`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HashTableOptions {
    /// Key comparison function selected by `:TEST`.
    pub test: HashTest,
    /// Weak-reference policy selected by `:WEAKNESS`.
    pub weakness: Weakness,
}

impl Default for HashTableOptions {
    fn default() -> Self {
        Self {
            test: HashTest::Eql,
            weakness: Weakness::None,
        }
    }
}

/// The two values returned by `GETHASH`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GethashResult {
    /// The stored value, or the caller-provided default.
    pub value: LispValue,
    /// Whether the key was present.
    pub present: bool,
}

fn object_error(error: ObjectError) -> LispError {
    LispError::from(error)
}

/// Allocate an empty hash table from typed options.
pub fn make_hash_table(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    options: HashTableOptions,
) -> Result<HashTableView, LispError> {
    HashTable::new(ctx, runtime, options.test, options.weakness)
        .map(HashTableView)
        .map_err(object_error)
}

/// Return a hash-table value and its presence flag.
pub fn gethash(
    ctx: &mut ThreadContext,
    table: HashTableView,
    key: LispValue,
    default: LispValue,
) -> Result<GethashResult, LispError> {
    let value = table.0.get(ctx, key.as_word()).map_err(object_error)?;
    Ok(match value {
        Some(value) => GethashResult {
            value: LispValue::from_word(value),
            present: true,
        },
        None => GethashResult {
            value: default,
            present: false,
        },
    })
}

/// Insert or replace a hash-table entry.
pub fn set_hash_value(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    table: HashTableView,
    key: LispValue,
    value: LispValue,
) -> Result<(), LispError> {
    table
        .0
        .insert(ctx, runtime, key.as_word(), value.as_word())
        .map_err(object_error)
}

/// Remove a key and report whether it was present.
pub fn remhash(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    table: HashTableView,
    key: LispValue,
) -> Result<bool, LispError> {
    table
        .0
        .remove(ctx, runtime, key.as_word())
        .map(|value| value.is_some())
        .map_err(object_error)
}

/// Remove every entry and return the number of removed entries.
pub fn clrhash(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    table: HashTableView,
) -> Result<usize, LispError> {
    let mut keys = Vec::new();
    table
        .0
        .for_each_entry(ctx, |key, _| keys.push(LispValue::from_word(key)))
        .map_err(object_error)?;
    let count = keys.len();
    for key in keys {
        table
            .0
            .remove(ctx, runtime, key.as_word())
            .map_err(object_error)?;
    }
    Ok(count)
}

/// Visit each live key/value pair without exposing raw ABI words.
pub fn maphash<F>(ctx: &ThreadContext, table: HashTableView, mut visit: F) -> Result<(), LispError>
where
    F: FnMut(LispValue, LispValue),
{
    table
        .0
        .for_each_entry(ctx, |key, value| {
            visit(LispValue::from_word(key), LispValue::from_word(value));
        })
        .map_err(object_error)
}

/// Return the table's comparison test.
pub fn hash_table_test(ctx: &ThreadContext, table: HashTableView) -> Result<HashTest, LispError> {
    table.0.test(ctx).map_err(object_error)
}

/// Return the table's weak-reference policy.
pub fn hash_table_weakness(
    ctx: &ThreadContext,
    table: HashTableView,
) -> Result<Weakness, LispError> {
    table.0.weakness(ctx).map_err(object_error)
}

/// Return the number of live entries.
pub fn hash_table_count(ctx: &ThreadContext, table: HashTableView) -> Result<usize, LispError> {
    table.0.count(ctx).map_err(object_error)
}

/// Return the number of allocated index slots.
pub fn hash_table_size(ctx: &ThreadContext, table: HashTableView) -> Result<usize, LispError> {
    table.0.capacity(ctx).map_err(object_error)
}

/// Compute the implementation's stable hash for a Lisp value.
#[must_use]
pub fn sxhash_value(value: LispValue) -> u64 {
    sxhash(value.as_word())
}
