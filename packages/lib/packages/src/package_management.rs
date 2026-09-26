use super::{list_items, package_arg, package_designator, string_designator, with_rooted_words};
use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Package, Parameter, ParameterType,
    Runtime, ThreadContext, Word, classify_object,
};

pub(super) const MAKE_PACKAGE_OPTIONS: Parameter = Parameter {
    name: ncl_object::BuiltinName::new("OPTIONS"),
    ty: ParameterType::Any,
};
const NEW_NAME: Parameter = Parameter {
    name: ncl_object::BuiltinName::new("NEW-NAME"),
    ty: ParameterType::StringDesignator,
};
pub(super) const NEW_NICKNAMES: Parameter = Parameter {
    name: ncl_object::BuiltinName::new("NEW-NICKNAMES"),
    ty: ParameterType::List,
};
pub(super) const RENAME_PACKAGE_PARAMETERS: &[Parameter] = &[super::PACKAGE, NEW_NAME];

pub(super) fn delete_package(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let package = package_arg(ctx, runtime, args, 0)?;
    Ok(if runtime.delete_package(ctx, package)? {
        Word::TRUE
    } else {
        Word::NIL
    })
}

pub(super) fn rename_package(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let package = package_arg(ctx, runtime, args, 0)?;
    let name = string_designator(ctx, args.required(1)?)?;
    let nicknames = args.get(2).unwrap_or(Word::NIL);
    for nickname in list_items(ctx, nicknames)? {
        let _ = string_designator(ctx, nickname)?;
    }
    runtime.rename_package(ctx, package, name.as_word(), nicknames)?;
    Ok(package.as_word())
}

pub(super) fn shadowing_import(
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
                Package::from_word(package[0]).shadowing_import(ctx, runtime, symbol)?;
            }
            Ok(Word::TRUE)
        })
    })
}

pub(super) fn make_package(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = string_designator(ctx, args.required(0)?)?;
    let name = (0..ncl_object::string_length(ctx, name.as_word())?)
        .map(|index| ncl_object::string_ref(ctx, name.as_word(), index))
        .collect::<Result<String, _>>()?;
    if runtime.find_package(ctx, &name).is_some() {
        return Err(ObjectError::PackageConflict);
    }
    let mut nicknames = Word::NIL;
    let mut use_packages = Vec::new();
    let mut index = 1;
    while index < args.len() {
        let key = args.get(index).ok_or(ObjectError::TypeError)?;
        let value = args.get(index + 1).ok_or(ObjectError::TypeError)?;
        let key = string_designator(ctx, key)?;
        let key = (0..ncl_object::string_length(ctx, key.as_word())?)
            .map(|offset| ncl_object::string_ref(ctx, key.as_word(), offset))
            .collect::<Result<String, _>>()?;
        match key.as_str() {
            "NICKNAMES" => {
                for nickname in list_items(ctx, value)? {
                    let nickname = string_designator(ctx, nickname)?.as_word();
                    nicknames = ncl_object::make_cons(ctx, runtime, nickname, nicknames)?;
                }
            }
            "USE" => {
                for used in list_items(ctx, value)? {
                    use_packages.push(package_designator(ctx, runtime, used)?.as_word());
                }
            }
            _ => return Err(ObjectError::TypeError),
        }
        index += 2;
    }
    with_rooted_words(ctx, &mut use_packages, |ctx, use_packages| {
        with_rooted_words(
            ctx,
            std::slice::from_mut(&mut nicknames),
            |ctx, nicknames| {
                let mut package = runtime.ensure_package(ctx, &name)?;
                with_rooted_words(ctx, std::slice::from_mut(&mut package), |ctx, package| {
                    let mut roots = [package[0], nicknames[0]];
                    with_rooted_words(ctx, &mut roots, |ctx, roots| {
                        let name_word = ncl_object::make_string(
                            ctx,
                            runtime,
                            &name.chars().collect::<Vec<_>>(),
                        )?;
                        let mut name_word = name_word;
                        with_rooted_words(
                            ctx,
                            std::slice::from_mut(&mut name_word),
                            |ctx, name_word| {
                                runtime.rename_package(
                                    ctx,
                                    Package::from_word(roots[0]),
                                    name_word[0],
                                    roots[1],
                                )?;
                                for used in use_packages.iter().copied() {
                                    Package::from_word(roots[0]).use_package(ctx, runtime, used)?;
                                }
                                Ok(roots[0])
                            },
                        )
                    })
                })
            },
        )
    })
}
