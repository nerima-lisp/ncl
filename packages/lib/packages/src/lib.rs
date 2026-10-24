//! Typed package-lock builtins.

mod introspection;
mod symbols;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FromLispArg, LambdaList, LispError, MultipleValues, ObjectError, ObjectRef,
    Package, PackageError, Parameter, ParameterType, Runtime, StringObject, ThreadContext, Word,
};

const PACKAGE: Parameter = Parameter {
    name: BuiltinName::new("PACKAGE"),
    ty: ParameterType::PackageDesignator,
};

const NICKNAME: Parameter = Parameter {
    name: BuiltinName::new("NICKNAME"),
    ty: ParameterType::StringDesignator,
};

const TARGET_PACKAGE: Parameter = Parameter {
    name: BuiltinName::new("PACKAGE"),
    ty: ParameterType::PackageDesignator,
};

const PACKAGE_DESIGNATOR: Parameter = Parameter {
    name: BuiltinName::new("PACKAGE-DESIGNATOR"),
    ty: ParameterType::PackageDesignator,
};

#[derive(Clone, Copy)]
struct NicknameDesignator(Word);

#[derive(Clone, Copy)]
struct PackageDesignatorArg(Word);

fn with_rooted_words<T>(
    ctx: &mut ThreadContext,
    words: &mut [Word],
    f: impl FnOnce(&mut ThreadContext, &mut [Word]) -> Result<T, LispError>,
) -> Result<T, LispError> {
    let tokens = words
        .iter_mut()
        .map(|word| ncl_object::push_root(ctx, word))
        .collect::<Vec<_>>();
    let result = f(ctx, words);
    let mut cleanup_error = None;
    for token in tokens.into_iter().rev() {
        if !ncl_object::pop_root(ctx, token) {
            cleanup_error = Some(LispError::Object(ObjectError::Layout));
        }
    }
    match (result, cleanup_error) {
        (Err(error), _) | (Ok(_), Some(error)) => Err(error),
        (Ok(value), None) => Ok(value),
    }
}

impl FromLispArg for NicknameDesignator {
    fn from_lisp_arg(ctx: &ThreadContext, word: Word) -> Result<Self, LispError> {
        if matches!(
            ncl_object::classify_object(ctx, word),
            ObjectRef::String(_) | ObjectRef::Symbol(_) | ObjectRef::Character(_)
        ) {
            Ok(Self(word))
        } else {
            Err(LispError::TypeError {
                datum: word,
                expected: ncl_object::ObjectType::String,
            })
        }
    }
}

impl FromLispArg for PackageDesignatorArg {
    fn from_lisp_arg(ctx: &ThreadContext, word: Word) -> Result<Self, LispError> {
        if matches!(
            ncl_object::classify_object(ctx, word),
            ObjectRef::Package(_)
                | ObjectRef::String(_)
                | ObjectRef::Symbol(_)
                | ObjectRef::Character(_)
        ) {
            Ok(Self(word))
        } else {
            Err(LispError::TypeError {
                datum: word,
                expected: ncl_object::ObjectType::Package,
            })
        }
    }
}

fn package_arg(ctx: &ThreadContext, args: BuiltinArgs<'_>) -> Result<Package, ObjectError> {
    let word = args.required(0)?;
    Package::try_from_word(ctx, word)
}

fn package_locked(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if package_arg(ctx, *args)?.is_locked(ctx)? {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn lock_package(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let package = package_arg(ctx, *args)?;
    package.set_locked(ctx, true)?;
    Ok(package.as_word())
}

fn unlock_package(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let package = package_arg(ctx, *args)?;
    package.set_locked(ctx, false)?;
    Ok(package.as_word())
}

fn designator_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    designator: NicknameDesignator,
) -> Result<StringObject, LispError> {
    let word = match ncl_object::classify_object(ctx, designator.0) {
        ObjectRef::String(_) => StringObject::from_word(designator.0),
        ObjectRef::Symbol(symbol) => StringObject::from_word(ncl_object::symbol_name(ctx, symbol)?),
        ObjectRef::Character(character) => {
            let character =
                char::from_u32(character).ok_or(LispError::Object(ObjectError::TypeError))?;
            StringObject::from_word(ncl_object::make_string(ctx, runtime, &[character])?)
        }
        _ => {
            return Err(LispError::TypeError {
                datum: designator.0,
                expected: ncl_object::ObjectType::String,
            });
        }
    };
    Ok(word)
}

fn package_designator(
    ctx: &ThreadContext,
    runtime: &Runtime,
    designator: PackageDesignatorArg,
) -> Result<Package, LispError> {
    if let ObjectRef::Package(package) = ncl_object::classify_object(ctx, designator.0) {
        return Ok(Package::from_word(package));
    }
    let name = introspection::string_designator_text(ctx, designator.0)?;
    runtime
        .find_package(ctx, &name)
        .map(Package::from_word)
        .ok_or(LispError::PackageError(PackageError::NotFound))
}

fn ensure_unlocked(ctx: &mut ThreadContext, package: Package) -> Result<(), LispError> {
    if package.is_locked(ctx)? {
        ctx.set_pending_lisp_error(LispError::PackageError(PackageError::Locked));
        return Err(LispError::PackageError(PackageError::Locked));
    }
    Ok(())
}

fn add_package_local_nickname_impl(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    nickname: NicknameDesignator,
    target: PackageDesignatorArg,
    package: PackageDesignatorArg,
) -> Result<Word, LispError> {
    let nickname = designator_string(ctx, runtime, nickname)?;
    let package = package_designator(ctx, runtime, package)?;
    let target = package_designator(ctx, runtime, target)?;
    let mut roots = [nickname.as_word(), target.as_word(), package.as_word()];
    with_rooted_words(ctx, &mut roots, |ctx, roots| {
        let nickname = StringObject::from_word(roots[0]);
        let package = Package::from_word(roots[2]);
        ensure_unlocked(ctx, package)?;
        if package.resolve_local_nickname(ctx, nickname)?.is_some() {
            return Err(LispError::PackageError(PackageError::Conflict));
        }
        let mut entry = ncl_object::make_cons(ctx, runtime, roots[0], roots[1])?;
        with_rooted_words(ctx, std::slice::from_mut(&mut entry), |ctx, entry| {
            let package_word = roots.get(2).copied().ok_or(ObjectError::Layout)?;
            let mut previous = Package::from_word(package_word).local_nicknames(ctx)?;
            with_rooted_words(ctx, std::slice::from_mut(&mut previous), |ctx, previous| {
                let entry_word = entry.first().copied().ok_or(ObjectError::Layout)?;
                let previous_word = previous.first().copied().ok_or(ObjectError::Layout)?;
                let entries = ncl_object::make_cons(ctx, runtime, entry_word, previous_word)?;
                let package_word = roots.get(2).copied().ok_or(ObjectError::Layout)?;
                Package::from_word(package_word).set_local_nicknames(ctx, entries)?;
                Ok(roots[0])
            })
        })
    })
}

fn remove_package_local_nickname_impl(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    nickname: NicknameDesignator,
    package: PackageDesignatorArg,
) -> Result<Word, LispError> {
    let nickname = designator_string(ctx, runtime, nickname)?;
    let package = package_designator(ctx, runtime, package)?;
    ensure_unlocked(ctx, package)?;
    let nickname_length = ncl_object::string_length(ctx, nickname.as_word())?;
    let mut entries = package.local_nicknames(ctx)?;
    let mut previous = Word::NIL;
    while entries != Word::NIL {
        let entry = ncl_object::car(ctx, entries)?;
        let next = ncl_object::cdr(ctx, entries)?;
        let entry_name = ncl_object::car(ctx, entry)?;
        let mut matches = ncl_object::string_length(ctx, entry_name)? == nickname_length;
        for index in 0..nickname_length {
            if !matches {
                break;
            }
            matches = ncl_object::string_ref(ctx, entry_name, index)?
                == ncl_object::string_ref(ctx, nickname.as_word(), index)?;
        }
        if matches {
            if previous == Word::NIL {
                package.set_local_nicknames(ctx, next)?;
            } else {
                ncl_object::rplacd(ctx, previous, next)?;
            }
            return Ok(Word::TRUE);
        }
        previous = entries;
        entries = next;
    }
    Ok(Word::NIL)
}

fn package_local_nicknames_impl(
    ctx: &ThreadContext,
    runtime: &Runtime,
    package: PackageDesignatorArg,
) -> Result<Word, LispError> {
    Ok(package_designator(ctx, runtime, package)?.local_nicknames(ctx)?)
}

fn add_package_local_nickname(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let nickname = NicknameDesignator::from_lisp_arg(ctx, args.required(0)?).map_err(|error| {
        ctx.set_pending_lisp_error(error);
        ObjectError::TypeError
    })?;
    let target = PackageDesignatorArg::from_lisp_arg(ctx, args.required(1)?).map_err(|error| {
        ctx.set_pending_lisp_error(error);
        ObjectError::TypeError
    })?;
    let package = PackageDesignatorArg::from_lisp_arg(ctx, args.required(2)?).map_err(|error| {
        ctx.set_pending_lisp_error(error);
        ObjectError::TypeError
    })?;
    values.clear();
    add_package_local_nickname_impl(ctx, runtime, nickname, target, package).map_err(|error| {
        ctx.set_pending_lisp_error(error);
        ObjectError::TypeError
    })
}
ncl_object::typed_builtin!(
    remove_package_local_nickname,
    remove_package_local_nickname_impl,
    (nickname: NicknameDesignator, package: PackageDesignatorArg)
);
ncl_object::typed_builtin!(
    package_local_nicknames,
    package_local_nicknames_impl,
    (package: PackageDesignatorArg)
);

const fn descriptor() -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(&[PACKAGE]),
        convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
    }
}

const fn add_descriptor() -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(&[NICKNAME, TARGET_PACKAGE, PACKAGE_DESIGNATOR]),
        convention: ncl_object::BuiltinConvention::Adapted,
    }
}

const fn remove_descriptor() -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(&[NICKNAME, PACKAGE_DESIGNATOR]),
        convention: ncl_object::BuiltinConvention::Adapted,
    }
}

const fn local_nicknames_descriptor() -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(&[PACKAGE_DESIGNATOR]),
        convention: ncl_object::BuiltinConvention::Adapted,
    }
}

/// Register the NCL package-lock extension operations.
///
/// # Errors
/// Returns an object-layer error if builtin registration fails.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    let descriptor = descriptor();
    for (name, function) in [
        (
            "PACKAGE-LOCKED-P",
            package_locked as ncl_object::RustBuiltin,
        ),
        ("LOCK-PACKAGE", lock_package as ncl_object::RustBuiltin),
        ("UNLOCK-PACKAGE", unlock_package as ncl_object::RustBuiltin),
    ] {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclExt, BuiltinName::new(name)),
            BuiltinImplementation::direct(descriptor, function),
        )?;
    }
    for (name, descriptor, function) in [
        (
            "ADD-PACKAGE-LOCAL-NICKNAME",
            add_descriptor(),
            add_package_local_nickname as ncl_object::RustBuiltin,
        ),
        (
            "REMOVE-PACKAGE-LOCAL-NICKNAME",
            remove_descriptor(),
            remove_package_local_nickname as ncl_object::RustBuiltin,
        ),
        (
            "PACKAGE-LOCAL-NICKNAMES",
            local_nicknames_descriptor(),
            package_local_nicknames as ncl_object::RustBuiltin,
        ),
    ] {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclExt, BuiltinName::new(name)),
            BuiltinImplementation::direct(descriptor, function),
        )?;
    }
    introspection::register(runtime)?;
    symbols::register(runtime)?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/lib.rs"]
mod tests;
