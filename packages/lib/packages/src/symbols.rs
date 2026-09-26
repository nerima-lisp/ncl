//! Common Lisp symbol and symbol-cell builtins.

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, MultipleValues, ObjectError, ObjectRef, Package, Parameter,
    ParameterType, Runtime, ThreadContext, Word, car, cdr, make_string, make_symbol,
    set_symbol_value, symbol_function, symbol_name, symbol_package, symbol_plist, symbol_value,
};

const SYMBOL: Parameter = Parameter {
    name: BuiltinName::new("SYMBOL"),
    ty: ParameterType::Any,
};
const VALUE: Parameter = Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ParameterType::Any,
};
const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};
const INDICATOR: Parameter = Parameter {
    name: BuiltinName::new("INDICATOR"),
    ty: ParameterType::Any,
};
const DEFAULT: Parameter = Parameter {
    name: BuiltinName::new("DEFAULT"),
    ty: ParameterType::Any,
};
const PREFIX: Parameter = Parameter {
    name: BuiltinName::new("PREFIX"),
    ty: ParameterType::Any,
};
const COPY_PROPERTIES: Parameter = Parameter {
    name: BuiltinName::new("COPY-PROPERTIES"),
    ty: ParameterType::Any,
};
const PACKAGE_PARAM: Parameter = Parameter {
    name: BuiltinName::new("PACKAGE"),
    ty: ParameterType::Any,
};
const COPY_REQUIRED: &[Parameter] = &[SYMBOL];
const COPY_OPTIONAL: &[Parameter] = &[COPY_PROPERTIES];
const SYMBOL_INDICATOR: &[Parameter] = &[SYMBOL, INDICATOR];
const SYMBOL_INDICATOR_DEFAULT: &[Parameter] = &[DEFAULT];
const SYMBOL_OBJECT: &[Parameter] = &[OBJECT];
const PREFIX_OPTIONAL: &[Parameter] = &[PREFIX];
const GENTEMP_OPTIONAL: &[Parameter] = &[PREFIX, PACKAGE_PARAM];

fn symbol(ctx: &ThreadContext, word: Word) -> Result<Word, ObjectError> {
    if matches!(ncl_object::classify_object(ctx, word), ObjectRef::Symbol(_)) {
        Ok(word)
    } else {
        Err(ObjectError::TypeError)
    }
}

fn text(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
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

const fn bool_word(value: bool) -> Word {
    if value { Word::TRUE } else { Word::NIL }
}

fn boundp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(bool_word(
        symbol_value(ctx, symbol_arg(ctx, args)?)? != Word::UNBOUND,
    ))
}

fn fboundp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(bool_word(
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

fn symbolp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(bool_word(matches!(
        ncl_object::classify_object(ctx, args.required(0)?),
        ObjectRef::Symbol(_)
    )))
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
    let ncl = runtime.ensure_package(ctx, "NCL")?;
    let generated = Package::from_word(ncl).gensym(ctx, runtime)?;
    let suffix = text(ctx, symbol_name(ctx, generated)?)?;
    let name = make_string(
        ctx,
        runtime,
        &format!("{prefix}{}", suffix.strip_prefix('G').unwrap_or(&suffix))
            .chars()
            .collect::<Vec<_>>(),
    )?;
    make_symbol(ctx, runtime, name)
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
        let generated = package.gensym(ctx, runtime)?;
        let suffix = text(ctx, symbol_name(ctx, generated)?)?;
        let name = format!("{prefix}{}", suffix.strip_prefix('G').unwrap_or(&suffix));
        let name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
        if package.find_symbol(ctx, name_word)?.is_some() {
            continue;
        }
        let (symbol, _) = package.intern(ctx, runtime, &name)?;
        return Ok(symbol);
    }
}

const fn descriptor(required: &'static [Parameter], optional: &'static [Parameter]) -> Builtin {
    Builtin {
        lambda_list: LambdaList::with_optional(required, optional),
        convention: ncl_object::BuiltinConvention::Adapted,
    }
}

/// Register Common Lisp symbol builtins.
#[allow(clippy::too_many_lines)]
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    let one = LambdaList::fixed(&[SYMBOL]);
    let two = LambdaList::fixed(&[SYMBOL, VALUE]);
    let entries: &[(&str, Builtin, ncl_object::RustBuiltin)] = &[
        (
            "BOUNDP",
            Builtin {
                lambda_list: one,
                convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
            },
            boundp,
        ),
        (
            "FBOUNDP",
            Builtin {
                lambda_list: one,
                convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
            },
            fboundp,
        ),
        (
            "COPY-SYMBOL",
            descriptor(COPY_REQUIRED, COPY_OPTIONAL),
            copy_symbol,
        ),
        (
            "FMAKUNBOUND",
            Builtin {
                lambda_list: one,
                convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
            },
            fmakunbound,
        ),
        (
            "MAKUNBOUND",
            Builtin {
                lambda_list: one,
                convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
            },
            makunbound,
        ),
        (
            "SET",
            Builtin {
                lambda_list: two,
                convention: ncl_object::BuiltinConvention::Direct(Arity::exact(2)),
            },
            set,
        ),
        (
            "SYMBOL-FUNCTION",
            Builtin {
                lambda_list: one,
                convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
            },
            symbol_function_builtin,
        ),
        (
            "SYMBOL-NAME",
            Builtin {
                lambda_list: one,
                convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
            },
            symbol_name_builtin,
        ),
        (
            "SYMBOL-PACKAGE",
            Builtin {
                lambda_list: one,
                convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
            },
            symbol_package_builtin,
        ),
        (
            "SYMBOL-PLIST",
            Builtin {
                lambda_list: one,
                convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
            },
            symbol_plist_builtin,
        ),
        (
            "SYMBOL-VALUE",
            Builtin {
                lambda_list: one,
                convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
            },
            symbol_value_builtin,
        ),
        (
            "SYMBOLP",
            Builtin {
                lambda_list: LambdaList::fixed(SYMBOL_OBJECT),
                convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
            },
            symbolp,
        ),
        (
            "GET",
            descriptor(SYMBOL_INDICATOR, SYMBOL_INDICATOR_DEFAULT),
            get,
        ),
        (
            "REMPROP",
            Builtin {
                lambda_list: LambdaList::fixed(SYMBOL_INDICATOR),
                convention: ncl_object::BuiltinConvention::Direct(Arity::exact(2)),
            },
            remprop,
        ),
        ("GENSYM", descriptor(&[], PREFIX_OPTIONAL), gensym),
        ("GENTEMP", descriptor(&[], GENTEMP_OPTIONAL), gentemp),
    ];
    for (name, descriptor, function) in entries {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(*descriptor, *function),
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/symbols.rs"]
mod tests;
