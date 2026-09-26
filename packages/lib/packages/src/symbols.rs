//! Common Lisp symbol and symbol-cell builtins.

#[path = "make_symbol.rs"]
mod make_symbol;
#[path = "predicates.rs"]
mod predicates;
#[path = "symbol_registration.rs"]
mod symbol_registration;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, MultipleValues, ObjectError, ObjectRef, Package, Parameter,
    ParameterType, Runtime, ThreadContext, Word, car, cdr, make_string, make_symbol,
    set_symbol_special, set_symbol_value, symbol_function, symbol_name, symbol_package,
    symbol_plist, symbol_value,
};

fn with_rooted_words<T>(
    ctx: &mut ThreadContext,
    words: &mut [Word],
    f: impl FnOnce(&mut ThreadContext, &mut [Word]) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let tokens = words
        .iter_mut()
        .map(|word| ncl_object::push_root(ctx, word))
        .collect::<Vec<_>>();
    let result = f(ctx, words);
    let mut cleanup_error = None;
    for token in tokens.into_iter().rev() {
        if !ncl_object::pop_root(ctx, token) {
            cleanup_error = Some(ObjectError::Layout);
        }
    }
    match (result, cleanup_error) {
        (Err(error), _) => Err(error),
        (Ok(_), Some(error)) => Err(error),
        (Ok(value), None) => Ok(value),
    }
}

fn symbol(ctx: &ThreadContext, word: Word) -> Result<Word, ObjectError> {
    if matches!(ncl_object::classify_object(ctx, word), ObjectRef::Symbol(_)) {
        Ok(word)
    } else {
        Err(ObjectError::TypeError)
    }
}

pub fn text(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    let word = match ncl_object::classify_object(ctx, word) {
        ObjectRef::String(_) => word,
        ObjectRef::Symbol(_) => symbol_name(ctx, word)?,
        ObjectRef::Character(character) => {
            return char::from_u32(character)
                .map(|c| c.to_string())
                .ok_or(ObjectError::Layout);
        }
        _ => return Err(ObjectError::TypeError),
    };
    let length = ncl_object::string_length(ctx, word)?;
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, word, index))
        .collect()
}

fn symbol_arg(ctx: &ThreadContext, args: &BuiltinArgs<'_>) -> Result<Word, ObjectError> {
    symbol(ctx, args.required(0)?)
}

fn boundp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(predicates::bool_word(
        symbol_value(ctx, symbol_arg(ctx, args)?)? != Word::UNBOUND,
    ))
}

fn fboundp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(predicates::bool_word(
        symbol_function(ctx, symbol_arg(ctx, args)?)? != Word::UNBOUND,
    ))
}

fn copy_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let source = symbol_arg(ctx, args)?;
    let name = symbol_name(ctx, source)?;
    let copy_properties = args.get(1).unwrap_or(Word::NIL) != Word::NIL;
    let copy = make_symbol(ctx, runtime, name)?;
    if copy_properties {
        ctx.write_object_slot(
            copy,
            ncl_object::symbol_offset::VALUE,
            symbol_value(ctx, source)?,
        )?;
        ctx.write_object_slot(
            copy,
            ncl_object::symbol_offset::FUNCTION,
            symbol_function(ctx, source)?,
        )?;
        ctx.write_object_slot(
            copy,
            ncl_object::symbol_offset::PLIST,
            symbol_plist(ctx, source)?,
        )?;
    }
    Ok(copy)
}

fn cell_set(
    ctx: &mut ThreadContext,
    args: &BuiltinArgs<'_>,
    slot: usize,
    value: Word,
) -> Result<Word, ObjectError> {
    let symbol = symbol_arg(ctx, args)?;
    ctx.write_object_slot(symbol, slot, value)?;
    Ok(symbol)
}

fn fmakunbound(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    cell_set(
        ctx,
        args,
        ncl_object::symbol_offset::FUNCTION,
        Word::UNBOUND,
    )
}

fn makunbound(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let symbol = symbol_arg(ctx, args)?;
    set_symbol_value(ctx, symbol, Word::UNBOUND)?;
    Ok(symbol)
}

fn set(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let symbol = symbol_arg(ctx, args)?;
    let value = args.required(1)?;
    set_symbol_value(ctx, symbol, value)?;
    Ok(value)
}

fn symbol_function_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    symbol_function(ctx, symbol_arg(ctx, args)?)
}

fn symbol_name_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    symbol_name(ctx, symbol_arg(ctx, args)?)
}

fn symbol_package_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    symbol_package(ctx, symbol_arg(ctx, args)?)
}

fn symbol_plist_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    symbol_plist(ctx, symbol_arg(ctx, args)?)
}

fn symbol_value_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    symbol_value(ctx, symbol_arg(ctx, args)?)
}

fn get(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let symbol = symbol_arg(ctx, args)?;
    let indicator = args.required(1)?;
    let mut plist = symbol_plist(ctx, symbol)?;
    while plist != Word::NIL {
        if car(ctx, plist)? == indicator {
            let pair = cdr(ctx, plist)?;
            return car(ctx, pair);
        }
        let pair = cdr(ctx, plist)?;
        plist = cdr(ctx, pair)?;
    }
    Ok(args.get(2).unwrap_or(Word::NIL))
}

fn remprop(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let symbol = symbol_arg(ctx, args)?;
    let indicator = args.required(1)?;
    let mut plist = symbol_plist(ctx, symbol)?;
    let mut previous = Word::NIL;
    while plist != Word::NIL {
        let pair = cdr(ctx, plist)?;
        let next = cdr(ctx, pair)?;
        if car(ctx, plist)? == indicator {
            if previous == Word::NIL {
                ctx.write_object_slot(symbol, ncl_object::symbol_offset::PLIST, next)?;
            } else {
                ncl_object::rplacd(ctx, previous, next)?;
            }
            return Ok(Word::TRUE);
        }
        previous = cdr(ctx, plist)?;
        plist = next;
    }
    Ok(Word::NIL)
}

fn gensym(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let prefix = args
        .get(0)
        .map(|word| text(ctx, word))
        .transpose()?
        .unwrap_or_else(|| "G".to_owned());
    let counter = next_gensym_counter(ctx, runtime)?;
    let name = make_string(
        ctx,
        runtime,
        &format!("{prefix}{counter}").chars().collect::<Vec<_>>(),
    )?;
    let mut roots = [name];
    with_rooted_words(ctx, &mut roots, |ctx, roots| {
        make_symbol(ctx, runtime, roots[0])
    })
}

fn next_gensym_counter(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<i64, ObjectError> {
    let common = Package::from_word(
        runtime
            .find_package(ctx, "COMMON-LISP")
            .ok_or(ObjectError::Layout)?,
    );
    let (counter_symbol, _) = common.intern(ctx, runtime, "*GENSYM-COUNTER*")?;
    let counter = symbol_value(ctx, counter_symbol)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?;
    let next = counter.checked_add(1).ok_or(ObjectError::Layout)?;
    set_symbol_value(ctx, counter_symbol, Word::fixnum(next))?;
    Ok(counter)
}

fn package_arg(ctx: &ThreadContext, runtime: &Runtime, word: Word) -> Result<Package, ObjectError> {
    match ncl_object::classify_object(ctx, word) {
        ObjectRef::Package(_) => Ok(Package::from_word(word)),
        ObjectRef::String(_) | ObjectRef::Symbol(_) => {
            let name = text(ctx, word)?;
            runtime
                .find_package(ctx, &name)
                .map(Package::from_word)
                .ok_or(ObjectError::TypeError)
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn gentemp(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let prefix = args
        .get(0)
        .map(|word| text(ctx, word))
        .transpose()?
        .unwrap_or_else(|| "T".to_owned());
    let package_word = if let Some(word) = args.get(1) {
        word
    } else {
        runtime.ensure_package(ctx, "COMMON-LISP-USER")?
    };
    let package = package_arg(ctx, runtime, package_word)?;
    loop {
        let counter = next_gensym_counter(ctx, runtime)?;
        let name = format!("{prefix}{counter}");
        let mut roots = [make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?];
        if with_rooted_words(ctx, &mut roots, |ctx, roots| {
            Ok(package.find_symbol(ctx, roots[0])?.is_some())
        })? {
            continue;
        }
        let (symbol, _) = package.intern(ctx, runtime, &name)?;
        return Ok(symbol);
    }
}

/// Register Common Lisp symbol builtins.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    symbol_registration::register(runtime)
}

#[cfg(test)]
#[path = "tests/symbols.rs"]
mod tests;
