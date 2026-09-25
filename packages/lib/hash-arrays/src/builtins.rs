//! Builtin adapters for hash-table operations.

use super::*;

fn sxhash_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    // SXHASH returns a non-negative fixnum at the Lisp boundary.  The object
    // API deliberately exposes the full machine-width hash, so retain the
    // low fixnum payload bits when adapting it to Word.
    let hash = sxhash_value(LispValue::from_word(args.required(0)?));
    let fixnum = hash & (i64::MAX as u64);
    Ok(Word::fixnum(
        i64::try_from(fixnum).map_err(|_| ObjectError::Layout)?,
    ))
}

fn boundary(error: LispError) -> ObjectError {
    match error {
        LispError::Object(error) => error,
        LispError::TypeError { .. } => ObjectError::TypeError,
        _ => ObjectError::TypeError,
    }
}

fn table(
    ctx: &ThreadContext,
    args: BuiltinArgs<'_>,
    index: usize,
) -> Result<HashTableView, ObjectError> {
    HashTableView::try_from_word(ctx, args.required(index)?).map_err(boundary)
}

fn hash_table_p(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let word = args.required(0)?;
    Ok(
        if matches!(
            classify_object(ctx, word),
            ncl_object::ObjectRef::HashTable(_)
        ) {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

fn hash_table_count_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let count = hash_table_count(ctx, table(ctx, *args, 0)?).map_err(boundary)?;
    Ok(Word::fixnum(
        i64::try_from(count).map_err(|_| ObjectError::Layout)?,
    ))
}

fn hash_table_size_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let size = hash_table_size(ctx, table(ctx, *args, 0)?).map_err(boundary)?;
    Ok(Word::fixnum(
        i64::try_from(size).map_err(|_| ObjectError::Layout)?,
    ))
}

fn hash_table_test_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(Word::fixnum(
        hash_table_test(ctx, table(ctx, *args, 0)?).map_err(boundary)? as i64,
    ))
}

fn hash_table_weakness_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(Word::fixnum(
        hash_table_weakness(ctx, table(ctx, *args, 0)?).map_err(boundary)? as i64,
    ))
}

fn maphash_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let function = ncl_object::FunctionObject::try_from(args.required(0)?)?;
    let table = table(ctx, *args, 1)?;
    let mut entries = Vec::new();
    table
        .0
        .for_each_entry(ctx, |key, value| entries.push((key, value)))?;
    for (key, value) in entries {
        runtime.call_builtin(ctx, function, &[key, value])?;
    }
    Ok(table.as_word())
}

fn symbol_named(ctx: &ThreadContext, word: Word, expected: &str) -> Result<bool, ObjectError> {
    if !matches!(classify_object(ctx, word), ncl_object::ObjectRef::Symbol(_)) {
        return Ok(false);
    }
    let name = symbol_name(ctx, word)?;
    let expected = expected.chars().collect::<Vec<_>>();
    if string_length(ctx, name)? != expected.len() {
        return Ok(false);
    }
    expected
        .into_iter()
        .enumerate()
        .try_fold(true, |matches, (index, expected)| {
            Ok(matches && string_ref(ctx, name, index)? == expected)
        })
}

fn make_hash_keyword(
    ctx: &ThreadContext,
    keyword: Word,
    value: Word,
    options: &mut HashTableOptions,
) -> Result<(), ObjectError> {
    if symbol_named(ctx, keyword, "TEST")? {
        options.test = if symbol_named(ctx, value, "EQ")? {
            HashTest::Eq
        } else if symbol_named(ctx, value, "EQL")? {
            HashTest::Eql
        } else if symbol_named(ctx, value, "EQUAL")? {
            HashTest::Equal
        } else if symbol_named(ctx, value, "EQUALP")? {
            HashTest::Equalp
        } else {
            return Err(ObjectError::TypeError);
        };
    } else if symbol_named(ctx, keyword, "WEAKNESS")? {
        options.weakness = if symbol_named(ctx, value, "KEY")? {
            Weakness::Key
        } else if symbol_named(ctx, value, "VALUE")? {
            Weakness::Value
        } else if symbol_named(ctx, value, "KEY-AND-VALUE")? {
            Weakness::KeyAndValue
        } else if symbol_named(ctx, value, "KEY-OR-VALUE")? {
            Weakness::KeyOrValue
        } else if symbol_named(ctx, value, "NIL")? {
            Weakness::None
        } else {
            return Err(ObjectError::TypeError);
        };
    } else if symbol_named(ctx, keyword, "REHASH-SIZE")?
        || symbol_named(ctx, keyword, "REHASH-THRESHOLD")?
    {
        // HashTable currently owns a fixed initial capacity and load-factor
        // policy.  Accept the standard options so callers can use the common
        // interface; the object API applies its own policy until configurable
        // rehash parameters are exposed.
        return Ok(());
    } else {
        return Err(ObjectError::TypeError);
    }
    Ok(())
}

fn make_hash_table_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() % 2 != 0 {
        return Err(ObjectError::TypeError);
    }
    let mut options = HashTableOptions::default();
    for pair in (0..args.len()).step_by(2) {
        make_hash_keyword(
            ctx,
            args.required(pair)?,
            args.required(pair + 1)?,
            &mut options,
        )?;
    }
    Ok(make_hash_table(ctx, runtime, options)
        .map_err(boundary)?
        .as_word())
}

fn gethash_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = gethash(
        ctx,
        table(ctx, *args, 1)?,
        LispValue::from_word(args.required(0)?),
        LispValue::from_word(args.required(2)?),
    )
    .map_err(boundary)?;
    values.set(&[
        result.value.as_word(),
        if result.present {
            Word::TRUE
        } else {
            Word::NIL
        },
    ]);
    Ok(result.value.as_word())
}

fn remhash_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(
        if remhash(
            ctx,
            runtime,
            table(ctx, *args, 1)?,
            LispValue::from_word(args.required(0)?),
        )
        .map_err(boundary)?
        {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

fn clrhash_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let table = table(ctx, *args, 0)?;
    clrhash(ctx, runtime, table).map_err(boundary)?;
    Ok(table.as_word())
}

/// Register the implemented hash-table operations.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    const ONE: &[Parameter] = &[Parameter {
        name: BuiltinName::new("table"),
        ty: ParameterType::Any,
    }];
    const KEY_TABLE: &[Parameter] = &[
        Parameter {
            name: BuiltinName::new("key"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("table"),
            ty: ParameterType::Any,
        },
    ];
    const GETHASH: &[Parameter] = &[
        Parameter {
            name: BuiltinName::new("key"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("table"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("default"),
            ty: ParameterType::Any,
        },
    ];
    const MAKE_HASH_TABLE_KEYS: &[Parameter] = &[
        Parameter {
            name: BuiltinName::new("TEST"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("REHASH-SIZE"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("REHASH-THRESHOLD"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("WEAKNESS"),
            ty: ParameterType::Any,
        },
    ];
    const MAPHASH: &[Parameter] = &[
        Parameter {
            name: BuiltinName::new("FUNCTION"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("TABLE"),
            ty: ParameterType::Any,
        },
    ];
    let entries: &[(BuiltinName, Builtin, ncl_object::RustBuiltin)] = &[
        (
            BuiltinName::new("HASH-TABLE-P"),
            Builtin {
                lambda_list: LambdaList::fixed(ONE),
                convention: BuiltinConvention::Direct(ncl_object::Arity::exact(1)),
            },
            hash_table_p,
        ),
        (
            BuiltinName::new("HASH-TABLE-COUNT"),
            Builtin {
                lambda_list: LambdaList::fixed(ONE),
                convention: BuiltinConvention::Direct(ncl_object::Arity::exact(1)),
            },
            hash_table_count_builtin,
        ),
        (
            BuiltinName::new("HASH-TABLE-SIZE"),
            Builtin {
                lambda_list: LambdaList::fixed(ONE),
                convention: BuiltinConvention::Direct(ncl_object::Arity::exact(1)),
            },
            hash_table_size_builtin,
        ),
        (
            BuiltinName::new("HASH-TABLE-TEST"),
            Builtin {
                lambda_list: LambdaList::fixed(ONE),
                convention: BuiltinConvention::Direct(ncl_object::Arity::exact(1)),
            },
            hash_table_test_builtin,
        ),
        (
            BuiltinName::new("HASH-TABLE-WEAKNESS"),
            Builtin {
                lambda_list: LambdaList::fixed(ONE),
                convention: BuiltinConvention::Direct(ncl_object::Arity::exact(1)),
            },
            hash_table_weakness_builtin,
        ),
        (
            BuiltinName::new("MAKE-HASH-TABLE"),
            Builtin {
                lambda_list: LambdaList::with_keys(&[], MAKE_HASH_TABLE_KEYS, false),
                convention: BuiltinConvention::Adapted,
            },
            make_hash_table_builtin,
        ),
        (
            BuiltinName::new("SXHASH"),
            Builtin {
                lambda_list: LambdaList::fixed(ONE),
                convention: BuiltinConvention::Direct(ncl_object::Arity::exact(1)),
            },
            sxhash_builtin,
        ),
        (
            BuiltinName::new("MAPHASH"),
            Builtin {
                lambda_list: LambdaList::fixed(MAPHASH),
                convention: BuiltinConvention::Direct(ncl_object::Arity::exact(2)),
            },
            maphash_builtin,
        ),
        (
            BuiltinName::new("GETHASH"),
            Builtin {
                lambda_list: LambdaList::fixed(GETHASH),
                convention: BuiltinConvention::Direct(ncl_object::Arity::exact(3)),
            },
            gethash_builtin,
        ),
        (
            BuiltinName::new("REMHASH"),
            Builtin {
                lambda_list: LambdaList::fixed(KEY_TABLE),
                convention: BuiltinConvention::Direct(ncl_object::Arity::exact(2)),
            },
            remhash_builtin,
        ),
        (
            BuiltinName::new("CLRHASH"),
            Builtin {
                lambda_list: LambdaList::fixed(ONE),
                convention: BuiltinConvention::Direct(ncl_object::Arity::exact(1)),
            },
            clrhash_builtin,
        ),
    ];
    for (name, descriptor, function) in entries {
        runtime.register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, *name),
            BuiltinImplementation::direct(*descriptor, *function),
        )?;
    }
    Ok(())
}
