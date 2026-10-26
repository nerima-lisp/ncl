//! Common Lisp package introspection and mutation builtins.

#[path = "package_management.rs"]
mod package_management;

mod designators;
mod register;
pub use designators::{package_designator, string_designator_text, string_designator_word};
pub use register::register;

mod intern_symbol;
use intern_symbol::{intern, status_word};

use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FindStatus, Instance, LambdaList, MultipleValues, ObjectError, ObjectRef,
    Package, Parameter, ParameterType, Runtime, StringObject, ThreadContext, Word, classify_object,
    slot_ref, symbol_name,
};
use std::collections::HashSet;

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
const FIND_PACKAGE_NAME: Parameter = Parameter {
    name: BuiltinName::new("NAME"),
    ty: ParameterType::Any,
};
const SYMBOLS: Parameter = Parameter {
    name: BuiltinName::new("SYMBOLS"),
    ty: ParameterType::List,
};
const ONE_PACKAGE: &[Parameter] = &[PACKAGE];
const ONE_OBJECT: &[Parameter] = &[OBJECT];
const ONE_NAME: &[Parameter] = &[NAME];
const ONE_FIND_PACKAGE_NAME: &[Parameter] = &[FIND_PACKAGE_NAME];
const ONE_SYMBOLS: &[Parameter] = &[SYMBOLS];
const ITERATION_MODE: Parameter = Parameter {
    name: BuiltinName::new("MODE"),
    ty: ParameterType::Any,
};
const ITERATION_PACKAGE: Parameter = Parameter {
    name: BuiltinName::new("PACKAGE"),
    ty: ParameterType::Any,
};
pub(super) const PACKAGE_ITERATION_PARAMETERS: &[Parameter] = &[ITERATION_MODE, ITERATION_PACKAGE];
pub fn package_arg(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    index: usize,
) -> Result<Package, ObjectError> {
    let word = if let Some(word) = args.get(index) {
        word
    } else {
        let common = runtime
            .find_package(ctx, "COMMON-LISP")
            .ok_or(ObjectError::Layout)?;
        let current_package = Package::from_word(common)
            .intern(ctx, runtime, "*PACKAGE*")?
            .0;
        ncl_object::symbol_value(ctx, current_package)?
    };
    package_designator(ctx, runtime, word)
}

pub fn list_items(ctx: &ThreadContext, mut list: Word) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();
    while list != Word::NIL {
        if !visited.insert(list.bits()) {
            return Err(ObjectError::TypeError);
        }
        result.push(ncl_object::car(ctx, list)?);
        list = ncl_object::cdr(ctx, list)?;
    }
    Ok(result)
}

fn string_designator(ctx: &ThreadContext, word: Word) -> Result<StringObject, ObjectError> {
    let object = classify_object(ctx, word);
    if let ObjectRef::String(_) = object {
        Ok(StringObject::from_word(word))
    } else if let ObjectRef::Symbol(symbol) = object {
        Ok(StringObject::from_word(symbol_name(ctx, symbol)?))
    } else {
        Err(ObjectError::TypeError)
    }
}

fn find_package(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let argument = args.required(0)?;
    if matches!(classify_object(ctx, argument), ObjectRef::Package(_)) {
        return Ok(argument);
    }
    let name = string_designator_text(ctx, argument)?;
    Ok(runtime.find_package(ctx, &name).unwrap_or(Word::NIL))
}

fn package_name(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    package_arg(ctx, runtime, args, 0)?.name(ctx)
}

fn package_nicknames(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    package_arg(ctx, runtime, args, 0)?.nicknames(ctx)
}

fn package_shadowing_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    package_arg(ctx, runtime, args, 0)?.shadowing_symbols(ctx)
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
    let name = string_designator_text(ctx, args.required(0)?)?
        .chars()
        .collect::<Vec<_>>();
    let name_length = name.len();
    let mut candidates = Vec::new();
    for package in runtime.all_packages(ctx)? {
        Package::from_word(package).for_each_symbol(ctx, |symbol| candidates.push(symbol))?;
    }
    let mut symbols = Vec::new();
    with_rooted_words(ctx, &mut candidates, |ctx, candidates| {
        for symbol in candidates.iter().copied() {
            if symbols.contains(&symbol) {
                continue;
            }
            let symbol_name = symbol_name(ctx, symbol)?;
            if ncl_object::string_length(ctx, symbol_name)? == name_length
                && name.iter().copied().enumerate().all(|(index, character)| {
                    ncl_object::string_ref(ctx, symbol_name, index) == Ok(character)
                })
            {
                symbols.push(symbol);
            }
        }
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

fn string_word_equals(
    ctx: &ThreadContext,
    word: Word,
    expected: &str,
) -> Result<bool, ObjectError> {
    let characters = expected.chars().collect::<Vec<_>>();
    if ncl_object::string_length(ctx, word)? != characters.len() {
        return Ok(false);
    }
    for (index, character) in characters.into_iter().enumerate() {
        if ncl_object::string_ref(ctx, word, index)? != character {
            return Ok(false);
        }
    }
    Ok(true)
}

fn symbol_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    symbols: &mut [Word],
) -> Result<Word, ObjectError> {
    with_rooted_words(ctx, symbols, |ctx, symbols| {
        let mut list = [Word::NIL];
        with_rooted_words(ctx, &mut list, |ctx, list| {
            for symbol in symbols.iter().rev().copied() {
                list[0] = ncl_object::make_cons(ctx, runtime, symbol, list[0])?;
            }
            Ok(list[0])
        })
    })
}

fn all_package_iteration_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<Word, ObjectError> {
    let mut symbols = Vec::new();
    for package in runtime.all_packages(ctx)? {
        Package::from_word(package).for_each_symbol(ctx, |symbol| symbols.push(symbol))?;
    }
    symbol_list(ctx, runtime, &mut symbols)
}

fn package_iteration_symbols_for(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    package: Package,
    external_only: bool,
) -> Result<Word, ObjectError> {
    let mut package_root = [package.as_word()];
    with_rooted_words(ctx, &mut package_root, |ctx, package_root| {
        let package = Package::from_word(package_root[0]);
        let mut local = Vec::new();
        package.for_each_symbol(ctx, |symbol| local.push(symbol))?;

        if external_only {
            let mut external = Vec::new();
            with_rooted_words(ctx, &mut local, |ctx, local| {
                for symbol in local.iter().copied() {
                    let name = symbol_name(ctx, symbol)?;
                    if let Some((found, FindStatus::External)) = package.find_symbol(ctx, name)? {
                        if found == symbol {
                            external.push(symbol);
                        }
                    }
                }
                Ok(())
            })?;
            return symbol_list(ctx, runtime, &mut external);
        }

        let mut inherited = Vec::new();
        for used in list_items(ctx, package.use_list(ctx)?)? {
            let used = Package::from_word(used);
            let mut candidates = Vec::new();
            used.for_each_symbol(ctx, |symbol| candidates.push(symbol))?;
            with_rooted_words(ctx, &mut candidates, |ctx, candidates| {
                for candidate in candidates.iter().copied() {
                    let name = symbol_name(ctx, candidate)?;
                    let Some((source, FindStatus::External)) = used.find_symbol(ctx, name)? else {
                        continue;
                    };
                    if source != candidate {
                        continue;
                    }
                    let Some((visible, FindStatus::Inherited)) = package.find_symbol(ctx, name)?
                    else {
                        continue;
                    };
                    if visible == candidate {
                        inherited.push(candidate);
                    }
                }
                Ok(())
            })?;
        }
        local.extend(inherited);
        symbol_list(ctx, runtime, &mut local)
    })
}

fn package_iteration_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mode = symbol_name(ctx, args.required(0)?)?;
    if string_word_equals(ctx, mode, "ALL")? {
        return all_package_iteration_symbols(ctx, runtime);
    }
    let external_only = if string_word_equals(ctx, mode, "EXTERNAL")? {
        true
    } else if string_word_equals(ctx, mode, "SYMBOLS")? {
        false
    } else {
        return Err(ObjectError::TypeError);
    };
    let package = package_arg(ctx, runtime, args, 1)?;
    package_iteration_symbols_for(ctx, runtime, package, external_only)
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
    let name = string_designator_text(ctx, name.as_word())?;
    let package = package_arg(ctx, runtime, args, 1)?;
    let name_word = ncl_object::make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    let found = package.find_symbol(ctx, name_word)?;
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

fn mutate_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    operation: fn(Package, &mut ThreadContext, &Runtime, Word) -> Result<bool, ObjectError>,
) -> Result<Word, ObjectError> {
    let mut symbols = list_items(ctx, args.required(0)?)?;
    with_rooted_words(ctx, &mut symbols, |ctx, symbols| {
        let mut package = package_arg(ctx, runtime, args, 1)?.as_word();
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
    let name = string_designator_text(ctx, args.required(0)?)?;
    let package = package_arg(ctx, runtime, args, 1)?;
    let name_word = ncl_object::make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    Ok(if package.unintern(ctx, runtime, name_word)? {
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
    with_rooted_words(ctx, &mut names, |ctx, names| {
        let mut package = package_arg(ctx, runtime, args, 1)?.as_word();
        with_rooted_words(ctx, std::slice::from_mut(&mut package), |ctx, package| {
            for name in names.iter().copied() {
                let name = string_designator_text(ctx, name)?;
                let name =
                    ncl_object::make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
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
    let mut used = package_designator_list(ctx, runtime, args.required(0)?)?;
    with_rooted_words(ctx, &mut used, |ctx, used| {
        let mut package = package_arg(ctx, runtime, args, 1)?.as_word();
        with_rooted_words(ctx, std::slice::from_mut(&mut package), |ctx, package| {
            for designator in used.iter().copied() {
                let used = package_designator(ctx, runtime, designator)?;
                let package_word = package.first().copied().ok_or(ObjectError::Layout)?;
                Package::from_word(package_word).use_package(ctx, runtime, used.as_word())?;
            }
            Ok(Word::TRUE)
        })
    })
}

fn unuse_package(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut used = package_designator_list(ctx, runtime, args.required(0)?)?;
    with_rooted_words(ctx, &mut used, |ctx, used| {
        let mut package = package_arg(ctx, runtime, args, 1)?.as_word();
        with_rooted_words(ctx, std::slice::from_mut(&mut package), |ctx, package| {
            let mut changed = false;
            for designator in used.iter().copied() {
                let used = package_designator(ctx, runtime, designator)?;
                let package_word = package.first().copied().ok_or(ObjectError::Layout)?;
                changed |= Package::from_word(package_word).unuse_package(ctx, used.as_word())?;
            }
            Ok(if changed { Word::TRUE } else { Word::NIL })
        })
    })
}

fn package_designator_list(
    ctx: &ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<Vec<Word>, ObjectError> {
    let designators = if word == Word::NIL {
        Vec::new()
    } else if word.is_cons() {
        list_items(ctx, word)?
    } else {
        vec![word]
    };
    for designator in designators.iter().copied() {
        package_designator(ctx, runtime, designator)?;
    }
    Ok(designators)
}

#[cfg(test)]
#[path = "tests/introspection.rs"]
mod tests;
