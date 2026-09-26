//! Common Lisp package introspection and mutation builtins.

#[path = "package_management.rs"]
mod package_management;

mod register;
pub use register::register;

use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FindStatus, Instance, LambdaList, MultipleValues, ObjectError, ObjectRef,
    Package, Parameter, ParameterType, Runtime, StringObject, ThreadContext, Word, classify_object,
    slot_ref, symbol_name,
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
        (Err(error), _) | (Ok(_), Some(error)) => Err(error),
        (Ok(value), None) => Ok(value),
    }
}

const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};
const PACKAGE: Parameter = Parameter {
    name: BuiltinName::new("PACKAGE"),
    ty: ParameterType::PackageDesignator,
};
const NAME: Parameter = Parameter {
    name: BuiltinName::new("NAME"),
    ty: ParameterType::StringDesignator,
};
const SYMBOLS: Parameter = Parameter {
    name: BuiltinName::new("SYMBOLS"),
    ty: ParameterType::List,
};
const ONE_PACKAGE: &[Parameter] = &[PACKAGE];
const ONE_OBJECT: &[Parameter] = &[OBJECT];
const NAME_PACKAGE: &[Parameter] = &[NAME, PACKAGE];
const SYMBOLS_PACKAGE: &[Parameter] = &[SYMBOLS, PACKAGE];
const PACKAGE_PACKAGE: &[Parameter] = &[PACKAGE, PACKAGE];
pub fn string_designator(ctx: &ThreadContext, word: Word) -> Result<StringObject, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::String(_) => Ok(StringObject::from_word(word)),
        ObjectRef::Symbol(symbol) => Ok(StringObject::from_word(symbol_name(ctx, symbol)?)),
        _ => Err(ObjectError::TypeError),
    }
}

pub fn package_designator(
    ctx: &ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<Package, ObjectError> {
    if let ObjectRef::Package(package) = classify_object(ctx, word) {
        return Ok(Package::from_word(package));
    }
    let name = string_designator(ctx, word)?;
    let length = ncl_object::string_length(ctx, name.as_word())?;
    let name = (0..length)
        .map(|index| ncl_object::string_ref(ctx, name.as_word(), index))
        .collect::<Result<String, _>>()?;
    runtime
        .find_package(ctx, &name)
        .map(Package::from_word)
        .ok_or(ObjectError::TypeError)
}

pub fn package_arg(
    ctx: &ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    index: usize,
) -> Result<Package, ObjectError> {
    package_designator(ctx, runtime, args.required(index)?)
}

pub fn list_items(ctx: &mut ThreadContext, mut list: Word) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    while list != Word::NIL {
        result.push(ncl_object::car(ctx, list)?);
        list = ncl_object::cdr(ctx, list)?;
    }
    Ok(result)
}

fn status_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    status: FindStatus,
) -> Result<Word, ObjectError> {
    let keyword = Package::from_word(
        runtime
            .find_package(ctx, "KEYWORD")
            .ok_or(ObjectError::Layout)?,
    );
    let name = match status {
        FindStatus::Internal => "INTERNAL",
        FindStatus::External => "EXTERNAL",
        FindStatus::Inherited => "INHERITED",
    };
    keyword.intern(ctx, runtime, name).map(|(symbol, _)| symbol)
}

fn find_package(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = string_designator(ctx, args.required(0)?)?;
    let length = ncl_object::string_length(ctx, name.as_word())?;
    let name = (0..length)
        .map(|index| ncl_object::string_ref(ctx, name.as_word(), index))
        .collect::<Result<String, _>>()?;
    Ok(runtime.find_package(ctx, &name).unwrap_or(Word::NIL))
}

fn package_name(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Package::try_from_word(ctx, args.required(0)?)?.name(ctx)
}

fn package_nicknames(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Package::try_from_word(ctx, args.required(0)?)?.nicknames(ctx)
}

fn package_shadowing_symbols(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Package::try_from_word(ctx, args.required(0)?)?.shadowing_symbols(ctx)
}

fn package_use_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    package_arg(ctx, runtime, args, 0)?.use_list(ctx)
}

fn package_used_by_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    package_arg(ctx, runtime, args, 0)?.used_by_list(ctx)
}

fn list_all_packages(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut packages = runtime.all_packages(ctx)?;
    with_rooted_words(ctx, &mut packages, |ctx, packages| {
        let mut list = [Word::NIL];
        with_rooted_words(ctx, &mut list, |ctx, list| {
            for package in packages.iter().rev().copied() {
                list[0] = ncl_object::make_cons(ctx, runtime, package, list[0])?;
            }
            Ok(list[0])
        })
    })
}

fn find_all_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut name = string_designator(ctx, args.required(0)?).map(StringObject::as_word)?;
    let name_length = ncl_object::string_length(ctx, name)?;
    let mut candidates = Vec::new();
    for package in runtime.all_packages(ctx)? {
        Package::from_word(package).for_each_symbol(ctx, |symbol| candidates.push(symbol))?;
    }
    let mut symbols = Vec::new();
    with_rooted_words(ctx, std::slice::from_mut(&mut name), |ctx, name| {
        with_rooted_words(ctx, &mut candidates, |ctx, candidates| {
            for symbol in candidates.iter().copied() {
                if symbols.contains(&symbol) {
                    continue;
                }
                let symbol_name = symbol_name(ctx, symbol)?;
                if ncl_object::string_length(ctx, symbol_name)? == name_length
                    && (0..name_length).all(|index| {
                        ncl_object::string_ref(ctx, symbol_name, index)
                            == ncl_object::string_ref(ctx, name[0], index)
                    })
                {
                    symbols.push(symbol);
                }
            }
            Ok(())
        })?;
        Ok(())
    })?;
    with_rooted_words(ctx, &mut symbols, |ctx, symbols| {
        let mut list = [Word::NIL];
        with_rooted_words(ctx, &mut list, |ctx, list| {
            for symbol in symbols.iter().rev().copied() {
                list[0] = ncl_object::make_cons(ctx, runtime, symbol, list[0])?;
            }
            Ok(list[0])
        })
    })
}

fn package_error_package(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    slot_ref(ctx, Instance::from_word(args.required(0)?), 0)
}

fn packagep(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(
        if matches!(
            classify_object(ctx, args.required(0)?),
            ObjectRef::Package(_)
        ) {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

fn find_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = string_designator(ctx, args.required(0)?)?;
    let package = package_arg(ctx, runtime, args, 1)?;
    let found = package.find_symbol(ctx, name.as_word())?;
    let Some((symbol, status)) = found else {
        values.set(&[Word::NIL, Word::NIL]);
        return Ok(Word::NIL);
    };
    let mut symbol = symbol;
    let status = with_rooted_words(ctx, std::slice::from_mut(&mut symbol), |ctx, _| {
        status_word(ctx, runtime, status)
    })?;
    values.set(&[symbol, status]);
    Ok(symbol)
}

fn intern(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = string_designator(ctx, args.required(0)?)?;
    let package = package_arg(ctx, runtime, args, 1)?;
    let length = ncl_object::string_length(ctx, name.as_word())?;
    let name = (0..length)
        .map(|index| ncl_object::string_ref(ctx, name.as_word(), index))
        .collect::<Result<String, _>>()?;
    let (symbol, status) = package.intern(ctx, runtime, &name)?;
    let mut symbol = symbol;
    let status = with_rooted_words(ctx, std::slice::from_mut(&mut symbol), |ctx, _| {
        status_word(ctx, runtime, status)
    })?;
    values.set(&[symbol, status]);
    Ok(symbol)
}

fn mutate_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    operation: fn(Package, &mut ThreadContext, &Runtime, Word) -> Result<bool, ObjectError>,
) -> Result<Word, ObjectError> {
    let mut symbols = list_items(ctx, args.required(0)?)?;
    let mut package = package_arg(ctx, runtime, args, 1)?.as_word();
    with_rooted_words(ctx, &mut symbols, |ctx, symbols| {
        with_rooted_words(ctx, std::slice::from_mut(&mut package), |ctx, package| {
            for symbol in symbols.iter().copied() {
                let name = match classify_object(ctx, symbol) {
                    ObjectRef::Symbol(symbol) => symbol_name(ctx, symbol)?,
                    _ => return Err(ObjectError::TypeError),
                };
                operation(Package::from_word(package[0]), ctx, runtime, name)?;
            }
            Ok(Word::TRUE)
        })
    })
}

fn export(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    mutate_symbols(ctx, runtime, args, Package::export)
}

fn unexport(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    mutate_symbols(ctx, runtime, args, Package::unexport)
}

fn unintern(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = string_designator(ctx, args.required(0)?)?;
    let package = package_arg(ctx, runtime, args, 1)?;
    Ok(if package.unintern(ctx, runtime, name.as_word())? {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn import(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut symbols = list_items(ctx, args.required(0)?)?;
    let mut package = package_arg(ctx, runtime, args, 1)?.as_word();
    with_rooted_words(ctx, &mut symbols, |ctx, symbols| {
        with_rooted_words(ctx, std::slice::from_mut(&mut package), |ctx, package| {
            for symbol in symbols.iter().copied() {
                if !matches!(classify_object(ctx, symbol), ObjectRef::Symbol(_)) {
                    return Err(ObjectError::TypeError);
                }
                let name = symbol_name(ctx, symbol)?;
                Package::from_word(package[0]).import(ctx, runtime, name, symbol)?;
            }
            Ok(Word::TRUE)
        })
    })
}

fn shadow(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut names = list_items(ctx, args.required(0)?)?;
    let mut package = package_arg(ctx, runtime, args, 1)?.as_word();
    with_rooted_words(ctx, &mut names, |ctx, names| {
        with_rooted_words(ctx, std::slice::from_mut(&mut package), |ctx, package| {
            for name in names.iter().copied() {
                let name = string_designator(ctx, name)?.as_word();
                Package::from_word(package[0]).shadow(ctx, runtime, name)?;
            }
            Ok(Word::TRUE)
        })
    })
}

fn use_package(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let used = package_arg(ctx, runtime, args, 0)?;
    let package = package_arg(ctx, runtime, args, 1)?;
    package.use_package(ctx, runtime, used.as_word())?;
    Ok(Word::TRUE)
}

fn unuse_package(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let used = package_arg(ctx, runtime, args, 0)?;
    let package = package_arg(ctx, runtime, args, 1)?;
    Ok(if package.unuse_package(ctx, used.as_word())? {
        Word::TRUE
    } else {
        Word::NIL
    })
}

#[cfg(test)]
#[path = "tests/introspection.rs"]
mod tests;
