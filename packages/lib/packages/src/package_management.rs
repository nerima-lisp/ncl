use super::{
    list_items, package_arg, package_designator, string_designator_text, string_designator_word,
    with_rooted_words,
};
use ncl_object::{
    BuiltinArgs, HeapRootedWord, MultipleValues, ObjectError, ObjectRef, Package, Parameter,
    ParameterType, Runtime, ThreadContext, Word, classify_object,
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
    let package = package_arg(ctx, runtime, args, 0)?.as_word();
    let mut roots = [package, Word::NIL];
    with_rooted_words(ctx, &mut roots, |ctx, roots| {
        let name = string_designator_word(ctx, runtime, args.required(1)?)?;
        *roots.get_mut(1).ok_or(ObjectError::Layout)? = name;
        let mut nicknames = list_items(ctx, args.get(2).unwrap_or(Word::NIL))?;
        with_rooted_words(ctx, &mut nicknames, |ctx, nicknames| {
            for nickname in nicknames.iter_mut() {
                *nickname = string_designator_word(ctx, runtime, *nickname)?;
            }

            let mut nickname_list = Word::NIL;
            with_rooted_words(
                ctx,
                std::slice::from_mut(&mut nickname_list),
                |ctx, list| {
                    for nickname in nicknames.iter().rev().copied() {
                        list[0] = ncl_object::make_cons(ctx, runtime, nickname, list[0])?; // check-added-lines: allow(index) from_mut above roots one nickname list.
                    }
                    runtime.rename_package(ctx, Package::from_word(roots[0]), roots[1], list[0])?; // check-added-lines: allow(index) roots has two words and list has one.
                    Ok(roots[0])
                },
            )
        })
    })
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
    let name = string_designator_text(ctx, args.required(0)?)?;
    if runtime.find_package(ctx, &name).is_some() {
        return Err(ObjectError::PackageConflict);
    }
    let mut package_data = [Word::NIL, Word::NIL];
    with_rooted_words(ctx, &mut package_data, |ctx, package_data| {
        let mut use_packages = Vec::<HeapRootedWord>::new();
        let mut index = 1;
        while index < args.len() {
            let key = args.get(index).ok_or(ObjectError::TypeError)?;
            let value = args.get(index + 1).ok_or(ObjectError::TypeError)?;
            let key = string_designator_text(ctx, key)?;
            if key == "USE" {
                let mut used_words = list_items(ctx, value)?;
                with_rooted_words(ctx, &mut used_words, |ctx, used_words| {
                    for used in used_words.iter().copied() {
                        let package = package_designator(ctx, runtime, used)?.as_word();
                        use_packages.push(HeapRootedWord::new(ctx, package)?);
                    }
                    Ok(())
                })?;
            } else if key == "DOCUMENTATION" {
                if value != Word::NIL
                    && !matches!(classify_object(ctx, value), ObjectRef::String(_))
                {
                    return Err(ObjectError::TypeError);
                }
                package_data[1] = value; // check-added-lines: allow(index) package_data is the fixed two-slot array above.
            } else if key != "NICKNAMES" && key != "SIZE" {
                return Err(ObjectError::TypeError);
            }
            index += 2;
        }
        let mut index = 1;
        while index < args.len() {
            let key = args.get(index).ok_or(ObjectError::TypeError)?;
            let value = args.get(index + 1).ok_or(ObjectError::TypeError)?;
            let key = string_designator_text(ctx, key)?;
            if key == "NICKNAMES" {
                append_package_nicknames(
                    ctx,
                    runtime,
                    std::slice::from_mut(&mut package_data[0]), // check-added-lines: allow(index) package_data is the fixed two-slot array above.
                    value,
                )?;
            }
            index += 2;
        }
        let mut package = runtime.ensure_package(ctx, &name)?;
        with_rooted_words(ctx, std::slice::from_mut(&mut package), |ctx, package| {
            let package_word = package[0]; // check-added-lines: allow(index) from_mut above roots exactly one package word.
            let nicknames = package_data[0]; // check-added-lines: allow(index) package_data is the fixed two-slot array above.
            let documentation = package_data[1]; // check-added-lines: allow(index) package_data is the fixed two-slot array above.
            let mut roots = [package_word, nicknames, documentation];
            with_rooted_words(ctx, &mut roots, |ctx, roots| {
                let name_word =
                    ncl_object::make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
                let mut name_word = name_word;
                with_rooted_words(
                    ctx,
                    std::slice::from_mut(&mut name_word),
                    |ctx, name_word| {
                        let package = roots[0]; // check-added-lines: allow(index) roots has these three values above.
                        let nicknames = roots[1]; // check-added-lines: allow(index) roots has these three values above.
                        let documentation = roots[2]; // check-added-lines: allow(index) roots has these three values above.
                        let name = name_word[0]; // check-added-lines: allow(index) from_mut above roots exactly one name word.
                        runtime.rename_package(
                            ctx,
                            Package::from_word(package),
                            name,
                            nicknames,
                        )?;
                        Package::from_word(package).set_documentation(ctx, documentation)?;
                        for used in &use_packages {
                            Package::from_word(package).use_package(ctx, runtime, used.get())?;
                        }
                        Ok(package)
                    },
                )
            })
        })
    })
}

fn append_package_nicknames(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    nicknames: &mut [Word],
    value: Word,
) -> Result<(), ObjectError> {
    let mut values = list_items(ctx, value)?;
    with_rooted_words(ctx, &mut values, |ctx, values| {
        for nickname in values.iter().copied() {
            let mut nickname = string_designator_word(ctx, runtime, nickname)?;
            with_rooted_words(ctx, std::slice::from_mut(&mut nickname), |ctx, nickname| {
                let nickname_word = nickname.first().copied().ok_or(ObjectError::Layout)?;
                let nicknames_tail = nicknames.first().copied().ok_or(ObjectError::Layout)?;
                let next_nicknames =
                    ncl_object::make_cons(ctx, runtime, nickname_word, nicknames_tail)?;
                *nicknames.first_mut().ok_or(ObjectError::Layout)? = next_nicknames;
                Ok(())
            })?;
        }
        Ok(())
    })
}
