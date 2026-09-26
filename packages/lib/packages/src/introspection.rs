//! Common Lisp package introspection and mutation builtins.

use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FindStatus, LambdaList, MultipleValues, ObjectError, ObjectRef, Package,
    Parameter, ParameterType, Runtime, StringObject, ThreadContext, Word, classify_object,
    symbol_name,
};

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

fn string_designator(ctx: &ThreadContext, word: Word) -> Result<StringObject, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::String(_) => Ok(StringObject::from_word(word)),
        ObjectRef::Symbol(symbol) => Ok(StringObject::from_word(symbol_name(ctx, symbol)?)),
        _ => Err(ObjectError::TypeError),
    }
}

fn package_designator(
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

fn package_arg(
    ctx: &ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    index: usize,
) -> Result<Package, ObjectError> {
    package_designator(ctx, runtime, args.required(index)?)
}

fn list_items(ctx: &mut ThreadContext, mut list: Word) -> Result<Vec<Word>, ObjectError> {
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
    let status = status_word(ctx, runtime, status)?;
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
    let status = status_word(ctx, runtime, status)?;
    values.set(&[symbol, status]);
    Ok(symbol)
}

fn mutate_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    operation: fn(Package, &mut ThreadContext, &Runtime, Word) -> Result<bool, ObjectError>,
) -> Result<Word, ObjectError> {
    let symbols = list_items(ctx, args.required(0)?)?;
    let package = package_arg(ctx, runtime, args, 1)?;
    for symbol in symbols {
        let name = match classify_object(ctx, symbol) {
            ObjectRef::Symbol(symbol) => symbol_name(ctx, symbol)?,
            _ => return Err(ObjectError::TypeError),
        };
        operation(package, ctx, runtime, name)?;
    }
    Ok(Word::TRUE)
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
    let symbols = list_items(ctx, args.required(0)?)?;
    let package = package_arg(ctx, runtime, args, 1)?;
    for symbol in symbols {
        if !matches!(classify_object(ctx, symbol), ObjectRef::Symbol(_)) {
            return Err(ObjectError::TypeError);
        }
        let name = symbol_name(ctx, symbol)?;
        package.import(ctx, runtime, name, symbol)?;
    }
    Ok(Word::TRUE)
}

fn shadow(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let names = list_items(ctx, args.required(0)?)?;
    let package = package_arg(ctx, runtime, args, 1)?;
    for name in names {
        let name = string_designator(ctx, name)?.as_word();
        package.shadow(ctx, runtime, name)?;
    }
    Ok(Word::TRUE)
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

const fn descriptor(required: &'static [Parameter]) -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(required),
        convention: BuiltinConvention::Adapted,
    }
}

/// Register package introspection and mutation operations backed by the object API.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for (name, params, function) in [
        (
            "FIND-PACKAGE",
            &[NAME][..],
            find_package as ncl_object::RustBuiltin,
        ),
        ("PACKAGE-NAME", ONE_PACKAGE, package_name),
        ("PACKAGE-NICKNAMES", ONE_PACKAGE, package_nicknames),
        (
            "PACKAGE-SHADOWING-SYMBOLS",
            ONE_PACKAGE,
            package_shadowing_symbols,
        ),
        ("PACKAGEP", ONE_OBJECT, packagep),
        ("FIND-SYMBOL", NAME_PACKAGE, find_symbol),
        ("INTERN", NAME_PACKAGE, intern),
        ("EXPORT", SYMBOLS_PACKAGE, export),
        ("UNEXPORT", SYMBOLS_PACKAGE, unexport),
        ("UNINTERN", NAME_PACKAGE, unintern),
        ("IMPORT", SYMBOLS_PACKAGE, import),
        ("SHADOW", SYMBOLS_PACKAGE, shadow),
        ("USE-PACKAGE", PACKAGE_PACKAGE, use_package),
        ("UNUSE-PACKAGE", PACKAGE_PACKAGE, unuse_package),
    ] {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(descriptor(params), function),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ncl_object::FunctionObject;

    #[test]
    fn introspection_and_mutation_round_trip() {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("thread: {error:?}"));
        register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));
        let package = runtime
            .ensure_package(&mut ctx, "N25-INTROSPECTION")
            .unwrap_or_else(|error| panic!("package: {error:?}"));
        let function = |ctx: &mut ThreadContext, name| {
            FunctionObject::try_from(
                runtime
                    .function(ctx, "COMMON-LISP", name)
                    .unwrap_or_else(|| panic!("{name}")),
            )
            .unwrap_or_else(|error| panic!("function: {error:?}"))
        };
        let name = ncl_object::make_string(&mut ctx, &runtime, &['A'])
            .unwrap_or_else(|error| panic!("name: {error:?}"));
        let intern = function(&mut ctx, "INTERN");
        let symbol = runtime
            .call_builtin(&mut ctx, intern, &[name, package])
            .unwrap_or_else(|error| panic!("intern: {error:?}"));
        assert_eq!(ctx.values().len(), 2);
        let package_name = function(&mut ctx, "PACKAGE-NAME");
        assert_eq!(
            runtime.call_builtin(&mut ctx, package_name, &[package]),
            Ok(ncl_object::Package::from_word(package)
                .name(&ctx)
                .unwrap_or_else(|error| panic!("name: {error:?}")))
        );
        let packagep = function(&mut ctx, "PACKAGEP");
        assert_eq!(
            runtime.call_builtin(&mut ctx, packagep, &[package]),
            Ok(Word::TRUE)
        );
        let export = function(&mut ctx, "EXPORT");
        let symbols = ncl_object::make_cons(&mut ctx, &runtime, symbol, Word::NIL)
            .unwrap_or_else(|error| panic!("list: {error:?}"));
        assert_eq!(
            runtime.call_builtin(&mut ctx, export, &[symbols, package]),
            Ok(Word::TRUE)
        );
        let find = function(&mut ctx, "FIND-SYMBOL");
        assert_eq!(
            runtime.call_builtin(&mut ctx, find, &[name, package]),
            Ok(symbol)
        );
        assert_eq!(ctx.values().len(), 2);
    }
}
