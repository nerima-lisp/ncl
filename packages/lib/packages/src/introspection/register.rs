use super::{
    NAME, ONE_FIND_PACKAGE_NAME, ONE_NAME, ONE_OBJECT, ONE_PACKAGE, ONE_SYMBOLS, export,
    find_all_symbols, find_package, find_symbol, import, intern, list_all_packages, list_items,
    package_arg, package_error_package, package_management, package_name, package_nicknames,
    package_shadowing_symbols, package_use_list, package_used_by_list, packagep, shadow, unexport,
    unintern, unuse_package, use_package, with_rooted_words,
};
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FindStatus, LambdaList, MultipleValues, ObjectError, Package, Parameter,
    ParameterType, Runtime, ThreadContext, Word, symbol_name,
};

const ITERATION_MODE: Parameter = Parameter {
    name: BuiltinName::new("MODE"),
    ty: ParameterType::Any,
};
const ITERATION_PACKAGE: Parameter = Parameter {
    name: BuiltinName::new("PACKAGE"),
    ty: ParameterType::Any,
};
const PACKAGE_ITERATION_PARAMETERS: &[Parameter] = &[ITERATION_MODE, ITERATION_PACKAGE];

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
    mut symbols: Vec<Word>,
) -> Result<Word, ObjectError> {
    let list_index = symbols.len();
    symbols.push(Word::NIL);
    with_rooted_words(ctx, &mut symbols, |ctx, roots| {
        for index in (0..list_index).rev() {
            let symbol = roots[index]; // check-added-lines: allow(index) index comes from the symbol range.
            let tail = roots[list_index]; // check-added-lines: allow(index) NIL tail is appended above.
            roots[list_index] = ncl_object::make_cons(ctx, runtime, symbol, tail)?; // check-added-lines: allow(index) NIL tail is appended above.
        }
        Ok(roots[list_index]) // check-added-lines: allow(index) NIL tail is appended above.
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
    symbol_list(ctx, runtime, symbols)
}

#[derive(Clone, Copy)]
enum PackageIterationMode {
    Accessible,
    Present,
    External,
}

fn package_iteration_symbols_for(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    package: Package,
    mode: PackageIterationMode,
) -> Result<Word, ObjectError> {
    let mut package_root = [package.as_word()];
    with_rooted_words(ctx, &mut package_root, |ctx, package_root| {
        let package = Package::from_word(package_root[0]); // check-added-lines: allow(index) package_root has one slot.
        let mut symbols = Vec::new();
        let mut local = Vec::new();
        package.for_each_symbol(ctx, |symbol| local.push(symbol))?;

        match mode {
            PackageIterationMode::External => {
                for symbol in local.iter().copied() {
                    let name = symbol_name(ctx, symbol)?;
                    if let Some((_, FindStatus::External)) = package.find_symbol(ctx, name)? {
                        symbols.push(symbol);
                    }
                }
            }
            PackageIterationMode::Present => symbols.extend(local),
            PackageIterationMode::Accessible => {
                symbols.extend(local);
                let use_list = package.use_list(ctx)?;
                for used in list_items(ctx, use_list)? {
                    let used = Package::from_word(used);
                    let mut candidates = Vec::new();
                    used.for_each_symbol(ctx, |symbol| candidates.push(symbol))?;
                    for candidate in candidates {
                        let name = symbol_name(ctx, candidate)?;
                        let Some((_, FindStatus::External)) = used.find_symbol(ctx, name)? else {
                            continue;
                        };
                        let Some((visible, FindStatus::Inherited)) =
                            package.find_symbol(ctx, name)?
                        else {
                            continue;
                        };
                        if visible == candidate {
                            symbols.push(candidate);
                        }
                    }
                }
            }
        }
        // Lookups above do not allocate Lisp objects; root the collected symbols before consing.
        symbol_list(ctx, runtime, symbols)
    })
}

pub(super) fn package_iteration_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mode = symbol_name(ctx, args.required(0)?)?;
    if string_word_equals(ctx, mode, "ALL")? {
        return all_package_iteration_symbols(ctx, runtime);
    }
    let iteration_mode = if string_word_equals(ctx, mode, "EXTERNAL")? {
        PackageIterationMode::External
    } else if string_word_equals(ctx, mode, "PRESENT")? {
        PackageIterationMode::Present
    } else if string_word_equals(ctx, mode, "SYMBOLS")? {
        PackageIterationMode::Accessible
    } else {
        return Err(ObjectError::TypeError);
    };
    let package = package_arg(ctx, runtime, args, 1)?;
    package_iteration_symbols_for(ctx, runtime, package, iteration_mode)
}

const NO_PARAMETERS: &[Parameter] = &[];
type BuiltinRegistration = (
    &'static str,
    &'static [Parameter],
    &'static [Parameter],
    ncl_object::RustBuiltin,
);
const REGISTRATIONS: &[BuiltinRegistration] = &[
    (
        "FIND-PACKAGE",
        ONE_FIND_PACKAGE_NAME,
        NO_PARAMETERS,
        find_package,
    ),
    ("PACKAGE-NAME", ONE_PACKAGE, NO_PARAMETERS, package_name),
    (
        "PACKAGE-NICKNAMES",
        ONE_PACKAGE,
        NO_PARAMETERS,
        package_nicknames,
    ),
    (
        "PACKAGE-SHADOWING-SYMBOLS",
        ONE_PACKAGE,
        NO_PARAMETERS,
        package_shadowing_symbols,
    ),
    (
        "PACKAGE-USE-LIST",
        ONE_PACKAGE,
        NO_PARAMETERS,
        package_use_list,
    ),
    (
        "PACKAGE-USED-BY-LIST",
        ONE_PACKAGE,
        NO_PARAMETERS,
        package_used_by_list,
    ),
    ("PACKAGEP", ONE_OBJECT, NO_PARAMETERS, packagep),
    (
        "LIST-ALL-PACKAGES",
        NO_PARAMETERS,
        NO_PARAMETERS,
        list_all_packages,
    ),
    (
        "FIND-ALL-SYMBOLS",
        ONE_NAME,
        NO_PARAMETERS,
        find_all_symbols,
    ),
    (
        "PACKAGE-ERROR-PACKAGE",
        ONE_OBJECT,
        NO_PARAMETERS,
        package_error_package,
    ),
    ("FIND-SYMBOL", ONE_NAME, ONE_PACKAGE, find_symbol),
    ("INTERN", ONE_NAME, ONE_PACKAGE, intern),
    ("EXPORT", ONE_SYMBOLS, ONE_PACKAGE, export),
    ("UNEXPORT", ONE_SYMBOLS, ONE_PACKAGE, unexport),
    ("UNINTERN", ONE_NAME, ONE_PACKAGE, unintern),
    ("IMPORT", ONE_OBJECT, ONE_PACKAGE, import),
    ("SHADOW", ONE_SYMBOLS, ONE_PACKAGE, shadow),
    ("USE-PACKAGE", ONE_OBJECT, ONE_PACKAGE, use_package),
    ("UNUSE-PACKAGE", ONE_OBJECT, ONE_PACKAGE, unuse_package),
    (
        "DELETE-PACKAGE",
        ONE_PACKAGE,
        NO_PARAMETERS,
        package_management::delete_package,
    ),
    (
        "SHADOWING-IMPORT",
        ONE_SYMBOLS,
        ONE_PACKAGE,
        package_management::shadowing_import,
    ),
];

const fn descriptor(required: &'static [Parameter], optional: &'static [Parameter]) -> Builtin {
    Builtin {
        lambda_list: LambdaList::with_optional(required, optional),
        convention: BuiltinConvention::Adapted,
    }
}

/// Register package introspection and mutation operations backed by the object API.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    let mut registrations = REGISTRATIONS
        .iter()
        .map(|&(name, required, optional, function)| {
            (
                BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
                BuiltinImplementation::direct(descriptor(required, optional), function),
            )
        })
        .chain([
            (
                BuiltinIdentifier::new(
                    BuiltinPackage::NclExt,
                    BuiltinName::new("%PACKAGE-ITERATION-SYMBOLS"),
                ),
                BuiltinImplementation::direct(
                    Builtin {
                        lambda_list: LambdaList::fixed(PACKAGE_ITERATION_PARAMETERS),
                        convention: BuiltinConvention::Adapted,
                    },
                    package_iteration_symbols,
                ),
            ),
            (
                BuiltinIdentifier::new(
                    BuiltinPackage::CommonLisp,
                    BuiltinName::new("MAKE-PACKAGE"),
                ),
                BuiltinImplementation::direct(
                    Builtin {
                        lambda_list: LambdaList::with_rest(
                            &[NAME],
                            package_management::MAKE_PACKAGE_OPTIONS,
                        ),
                        convention: BuiltinConvention::Adapted,
                    },
                    package_management::make_package,
                ),
            ),
            (
                BuiltinIdentifier::new(
                    BuiltinPackage::CommonLisp,
                    BuiltinName::new("RENAME-PACKAGE"),
                ),
                BuiltinImplementation::direct(
                    Builtin {
                        lambda_list: LambdaList::with_optional(
                            package_management::RENAME_PACKAGE_PARAMETERS,
                            &[package_management::NEW_NICKNAMES],
                        ),
                        convention: BuiltinConvention::Adapted,
                    },
                    package_management::rename_package,
                ),
            ),
        ]);
    registrations.try_for_each(|(identifier, implementation)| {
        runtime
            .register_builtin(&mut ctx, identifier, implementation)
            .map(|_| ())
    })
}

#[cfg(test)]
#[path = "../tests/introspection_register.rs"]
mod tests;
