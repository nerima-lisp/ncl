//! Hash-table operations and builtin registration.

use ncl_object::{
    Builtin, BuiltinImplementation, FunctionObject, MultipleValues, ObjectError, ObjectRef,
    Runtime, ThreadContext, Word, classify_object,
};

mod arrays;
use ncl_object::hash_table::{HashTable, HashTest};

const COMMON_LISP: &str = "COMMON-LISP";
const HASH_FUNCTIONS: &[&str] = &[
    "CLRHASH",
    "GETHASH",
    "HASH-TABLE-COUNT",
    "HASH-TABLE-P",
    "HASH-TABLE-REHASH-SIZE",
    "HASH-TABLE-REHASH-THRESHOLD",
    "HASH-TABLE-SIZE",
    "HASH-TABLE-TEST",
    "MAKE-HASH-TABLE",
    "MAPHASH",
    "REMHASH",
];

/// Register all owned hash-table functions.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for &name in HASH_FUNCTIONS {
        let descriptor = Builtin {
            arity: 0,
            direct: false,
            lambda_list: "&rest args",
        };
        let implementation = match name {
            "GETHASH" => BuiltinImplementation::direct_with_runtime(descriptor, gethash_builtin),
            "HASH-TABLE-COUNT" => {
                BuiltinImplementation::direct_with_runtime(descriptor, hash_table_count_builtin)
            }
            "HASH-TABLE-P" => {
                BuiltinImplementation::direct_with_runtime(descriptor, hash_table_p_builtin)
            }
            "HASH-TABLE-SIZE" => {
                BuiltinImplementation::direct_with_runtime(descriptor, hash_table_size_builtin)
            }
            "HASH-TABLE-TEST" => {
                BuiltinImplementation::direct_with_runtime(descriptor, hash_table_test_builtin)
            }
            "MAKE-HASH-TABLE" => {
                BuiltinImplementation::direct_with_runtime(descriptor, make_hash_table_builtin)
            }
            "REMHASH" => BuiltinImplementation::direct_with_runtime(descriptor, remhash_builtin),
            "CLRHASH" => BuiltinImplementation::direct_with_runtime(descriptor, clrhash_builtin),
            "MAPHASH" => BuiltinImplementation::direct_with_runtime(descriptor, maphash_builtin),
            "HASH-TABLE-REHASH-SIZE" => {
                BuiltinImplementation::direct_with_runtime(descriptor, rehash_size_builtin)
            }
            "HASH-TABLE-REHASH-THRESHOLD" => {
                BuiltinImplementation::direct_with_runtime(descriptor, rehash_threshold_builtin)
            }
            _ => unreachable!(),
        };
        runtime.register_builtin(&mut ctx, COMMON_LISP, name, implementation)?;
    }
    arrays::register(runtime)?;
    Ok(())
}

/// Allocate a non-weak hash table with the requested comparison test.
pub fn make_hash_table(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    test: HashTest,
) -> Result<Word, ObjectError> {
    Ok(HashTable::new(ctx, runtime, test, ncl_object::hash_table::Weakness::None)?.as_word())
}

/// Return the value and presence flag for `GETHASH`.
pub fn gethash(
    ctx: &mut ThreadContext,
    key: Word,
    table: Word,
    default: Word,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let table = as_hash_table(ctx, table)?;
    let value = table.get(ctx, key)?;
    values.set(&[
        value.unwrap_or(default),
        if value.is_some() {
            Word::TRUE
        } else {
            Word::NIL
        },
    ]);
    Ok(value.unwrap_or(default))
}

/// Remove a key and return true when it was present.
pub fn remhash(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    key: Word,
    table: Word,
) -> Result<Word, ObjectError> {
    Ok(
        if as_hash_table(ctx, table)?
            .remove(ctx, runtime, key)?
            .is_some()
        {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

/// Remove all entries and return the table.
pub fn clrhash(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    table: Word,
) -> Result<Word, ObjectError> {
    let table_ref = as_hash_table(ctx, table)?;
    let mut keys = Vec::new();
    table_ref.for_each_entry(ctx, |key, _| keys.push(key))?;
    for key in keys {
        table_ref.remove(ctx, runtime, key)?;
    }
    Ok(table)
}

/// Call a registered builtin with each key/value pair and return NIL.
pub fn maphash(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    table: Word,
) -> Result<Word, ObjectError> {
    let table_ref = as_hash_table(ctx, table)?;
    let mut entries = Vec::new();
    table_ref.for_each_entry(ctx, |key, value| entries.push((key, value)))?;
    for (key, value) in entries {
        runtime.call_builtin(ctx, FunctionObject::from(function), &[key, value])?;
    }
    Ok(Word::NIL)
}

fn as_hash_table(ctx: &ThreadContext, word: Word) -> Result<HashTable, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::HashTable(word) => Ok(HashTable::from(word)),
        _ => Err(ObjectError::TypeError),
    }
}

fn gethash_builtin(
    _: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 3 {
        return Err(ObjectError::TypeError);
    }
    gethash(ctx, args[0], args[1], args[2], values)
}
fn hash_table_count_builtin(
    _: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    Ok(Word::fixnum(
        i64::try_from(as_hash_table(ctx, args[0])?.count(ctx)?).map_err(|_| ObjectError::Layout)?,
    ))
}
fn hash_table_p_builtin(
    _: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    Ok(
        if matches!(classify_object(ctx, args[0]), ObjectRef::HashTable(_)) {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}
fn hash_table_size_builtin(
    _: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    Ok(Word::fixnum(
        i64::try_from(as_hash_table(ctx, args[0])?.capacity(ctx)?)
            .map_err(|_| ObjectError::Layout)?,
    ))
}
fn hash_table_test_builtin(
    _: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    Ok(Word::fixnum(as_hash_table(ctx, args[0])?.test(ctx)? as i64))
}
fn make_hash_table_builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let test = match args.first().and_then(|word| word.as_fixnum()) {
        None | Some(2) => HashTest::Equal,
        Some(0) => HashTest::Eq,
        Some(1) => HashTest::Eql,
        Some(3) => HashTest::Equalp,
        Some(_) => return Err(ObjectError::TypeError),
    };
    make_hash_table(ctx, runtime, test)
}
fn remhash_builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    remhash(ctx, runtime, args[0], args[1])
}
fn clrhash_builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    clrhash(ctx, runtime, args[0])
}
fn maphash_builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    maphash(ctx, runtime, args[0], args[1])
}
fn rehash_size_builtin(
    _: &Runtime,
    _: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    Ok(Word::fixnum(2))
}
fn rehash_threshold_builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    Ok(ncl_object::make_double(ctx, runtime, 0.875)?.as_word())
}
