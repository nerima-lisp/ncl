use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::package::{nil, truth};
use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    classify_object, make_double, pop_root, push_root, BuiltinArgs, BuiltinFunctionCaller,
    BuiltinName, FunctionArguments, FunctionCaller, LambdaList, MultipleValues, ObjectError,
    ObjectRef, Package, Parameter, ParameterType, Runtime, ThreadContext, Word,
};

use super::{register_one, symbol_text};

const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};
const KEY: Parameter = Parameter {
    name: BuiltinName::new("KEY"),
    ty: ParameterType::Any,
};
const TABLE: Parameter = Parameter {
    name: BuiltinName::new("TABLE"),
    ty: ParameterType::Any,
};
const VALUE: Parameter = Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ParameterType::Any,
};
const CALLBACK: Parameter = Parameter {
    name: BuiltinName::new("FUNCTION"),
    ty: ParameterType::FunctionDesignator,
};
const OPTIONS: Parameter = Parameter {
    name: BuiltinName::new("OPTIONS"),
    ty: ParameterType::Any,
};

fn table(ctx: &ThreadContext, word: Word) -> Result<HashTable, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::HashTable(word) => Ok(HashTable::from_word(word)),
        _ => Err(ObjectError::TypeError),
    }
}

fn decode_test(ctx: &ThreadContext, word: Word) -> Result<HashTest, ObjectError> {
    match symbol_text(ctx, word)?.to_ascii_uppercase().as_str() {
        "EQ" => Ok(HashTest::Eq),
        "EQL" => Ok(HashTest::Eql),
        "EQUAL" => Ok(HashTest::Equal),
        "EQUALP" => Ok(HashTest::Equalp),
        _ => Err(ObjectError::TypeError),
    }
}

fn decode_weakness(ctx: &ThreadContext, word: Word) -> Result<Weakness, ObjectError> {
    if word == Word::NIL {
        return Ok(Weakness::None);
    }
    match symbol_text(ctx, word)?.to_ascii_uppercase().as_str() {
        "KEY" => Ok(Weakness::Key),
        "VALUE" => Ok(Weakness::Value),
        "KEY-AND-VALUE" => Ok(Weakness::KeyAndValue),
        "KEY-OR-VALUE" => Ok(Weakness::KeyOrValue),
        _ => Err(ObjectError::TypeError),
    }
}

fn make_hash_table_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut test = HashTest::Eql;
    let mut weak = Weakness::None;
    if !args.len().is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    for pair in args.as_slice().as_chunks::<2>().0 {
        match symbol_text(ctx, pair[0])?
            .to_ascii_uppercase()
            .trim_start_matches(':')
        {
            "TEST" => test = decode_test(ctx, pair[1])?,
            "WEAKNESS" => weak = decode_weakness(ctx, pair[1])?,
            // SIZE is not exposed by the heap HashTable API, so it is rejected
            // rather than registered as a misleading placeholder.
            _ => return Err(ObjectError::TypeError),
        }
    }
    Ok(HashTable::new(ctx, runtime, test, weak)?.as_word())
}

fn gethash_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let key = args.required(0)?;
    let table = table(ctx, args.required(1)?)?;
    let default = args.get(2).unwrap_or(Word::NIL);
    let result = table.get(ctx, key)?;
    let (value, present) = result.map_or((default, nil()), |value| (value, truth()));
    values.set(&[value, present]);
    Ok(value)
}

fn remhash_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let table = table(ctx, args.required(1)?)?;
    Ok(
        if table.remove(ctx, runtime, args.required(0)?)?.is_some() {
            truth()
        } else {
            nil()
        },
    )
}

fn clrhash_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let word = args.required(0)?;
    let table = table(ctx, word)?;
    let mut keys = Vec::new();
    table.for_each_entry(ctx, |key, _| keys.push(key))?;
    for key in keys {
        table.remove(ctx, runtime, key)?;
    }
    Ok(word)
}

fn maphash_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let callback = FunctionDesignator::try_from_word(ctx, args.required(0)?)?;
    let table = table(ctx, args.required(1)?)?;
    let mut entries = Vec::new();
    table.for_each_entry(ctx, |key, value| entries.push((key, value)))?;
    for (key, value) in entries {
        let mut callback_args = <[Word; 2]>::from((key, value));
        let key_token = push_root(ctx, &mut callback_args[0]);
        let value_token = push_root(ctx, &mut callback_args[1]);
        let mut caller = BuiltinFunctionCaller;
        let mut values = MultipleValues::new();
        let result = caller.call_function(
            ctx,
            runtime,
            callback,
            FunctionArguments::new(&callback_args),
            &mut values,
        );
        let value_popped = pop_root(ctx, value_token);
        let key_popped = pop_root(ctx, key_token);
        if !value_popped || !key_popped {
            return Err(ObjectError::Layout);
        }
        result?;
    }
    Ok(nil())
}

fn hash_table_rehash_size_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    table(ctx, args.required(0)?)?;
    Ok(make_double(ctx, runtime, 1.5)?.as_word())
}

fn hash_table_rehash_threshold_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    table(ctx, args.required(0)?)?;
    Ok(make_double(ctx, runtime, 0.75)?.as_word())
}

fn hash_table_p_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(
        if matches!(
            classify_object(ctx, args.required(0)?),
            ObjectRef::HashTable(_)
        ) {
            truth()
        } else {
            nil()
        },
    )
}

fn hash_table_count_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let count = table(ctx, args.required(0)?)?.count(ctx)?;
    Ok(Word::fixnum(
        i64::try_from(count).map_err(|_| ObjectError::TypeError)?,
    ))
}

fn hash_table_size_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let size = table(ctx, args.required(0)?)?.capacity(ctx)?;
    Ok(Word::fixnum(
        i64::try_from(size).map_err(|_| ObjectError::TypeError)?,
    ))
}

fn hash_table_test_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = match table(ctx, args.required(0)?)?.test(ctx)? {
        HashTest::Eq => "EQ",
        HashTest::Eql => "EQL",
        HashTest::Equal => "EQUAL",
        HashTest::Equalp => "EQUALP",
    };
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    Ok(Package::from_word(package).intern(ctx, runtime, name)?.0)
}

fn sxhash_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let hash = ncl_object::hash_table::sxhash(args.required(0)?);
    Ok(Word::fixnum((hash & (i64::MAX as u64)).cast_signed()))
}

pub fn register(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    register_one(
        runtime,
        ctx,
        "MAKE-HASH-TABLE",
        LambdaList::with_rest(&[], OPTIONS),
        make_hash_table_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "GETHASH",
        LambdaList::with_optional(&[KEY, TABLE], &[VALUE]),
        gethash_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "REMHASH",
        LambdaList::fixed(&[KEY, TABLE]),
        remhash_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "CLRHASH",
        LambdaList::fixed(&[TABLE]),
        clrhash_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "MAPHASH",
        LambdaList::fixed(&[CALLBACK, TABLE]),
        maphash_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "HASH-TABLE-P",
        LambdaList::fixed(&[OBJECT]),
        hash_table_p_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "HASH-TABLE-COUNT",
        LambdaList::fixed(&[TABLE]),
        hash_table_count_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "HASH-TABLE-SIZE",
        LambdaList::fixed(&[TABLE]),
        hash_table_size_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "HASH-TABLE-REHASH-SIZE",
        LambdaList::fixed(&[TABLE]),
        hash_table_rehash_size_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "HASH-TABLE-REHASH-THRESHOLD",
        LambdaList::fixed(&[TABLE]),
        hash_table_rehash_threshold_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "HASH-TABLE-TEST",
        LambdaList::fixed(&[TABLE]),
        hash_table_test_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "SXHASH",
        LambdaList::fixed(&[OBJECT]),
        sxhash_builtin,
    )?;
    Ok(())
}
