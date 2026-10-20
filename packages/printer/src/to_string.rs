use std::collections::HashSet;

use ncl_object::{
    Bignum, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, ObjectError, ObjectRef, Parameter, ParameterType,
    Runtime, RustBuiltin, ThreadContext, Word, bignum_limbs, bignum_sign, classify_object,
    symbol_name, symbol_package, with_root,
};

use crate::{PrintError, PrintOptions, write_to_string};

const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};
const OPTIONS: Parameter = Parameter {
    name: BuiltinName::new("OPTIONS"),
    ty: ParameterType::Any,
};

pub fn register(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Result<(), ObjectError> {
    let (builtin_name, lambda_list, function): (&'static str, LambdaList, RustBuiltin) =
        if name == "PRINC-TO-STRING" {
            (
                "PRINC-TO-STRING",
                LambdaList::fixed(&[OBJECT]),
                princ_to_string,
            )
        } else if name == "PRIN1-TO-STRING" {
            (
                "PRIN1-TO-STRING",
                LambdaList::fixed(&[OBJECT]),
                prin1_to_string,
            )
        } else if name == "WRITE-TO-STRING" {
            (
                "WRITE-TO-STRING",
                LambdaList::with_rest(&[OBJECT], OPTIONS),
                write_to_string_builtin,
            )
        } else {
            return Err(ObjectError::Layout); // check-added-lines: allow(wildcard) reject names outside this registration surface
        };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(builtin_name)),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list,
                convention: BuiltinConvention::Adapted,
            },
            function,
            identity_arguments,
        ),
    )?;
    Ok(())
}

fn princ_to_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    to_string_builtin(ctx, runtime, args, false, Some(false))
}

fn prin1_to_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    to_string_builtin(ctx, runtime, args, true, None)
}

fn write_to_string_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let mut object = args.required(0)?;
    with_root(ctx, &mut object, |ctx, object| {
        let options = write_options(ctx, runtime, args)?;
        write_to_string(ctx, runtime, object.get(), &options).map_err(|error| print_error(&error))
    })
}

fn to_string_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    escape: bool,
    readably: Option<bool>,
) -> Result<Word, ObjectError> {
    let mut object = args.required(0)?;
    with_root(ctx, &mut object, |ctx, object| {
        let mut options = PrintOptions::from_specials(ctx, runtime).with_escape(escape);
        if let Some(readably) = readably {
            options = options.with_readably(readably);
        }
        write_to_string(ctx, runtime, object.get(), &options).map_err(|error| print_error(&error))
    })
}

fn write_options(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
) -> Result<PrintOptions, ObjectError> {
    let Some(option_count) = args.len().checked_sub(1) else {
        return Err(ObjectError::TypeError);
    };
    if option_count % 2 != 0 {
        return Err(ObjectError::TypeError);
    }
    let mut options = PrintOptions::from_specials(ctx, runtime);
    let allow_other_keys = allow_other_keys(ctx, runtime, args)?;
    let mut supplied = HashSet::new();
    for index in (1..args.len()).step_by(2) {
        let key = args.get(index).ok_or(ObjectError::TypeError)?;
        let value = args.get(index + 1).ok_or(ObjectError::TypeError)?;
        let Some(name) = option_name(ctx, runtime, key)? else {
            if allow_other_keys {
                continue;
            }
            return Err(ObjectError::TypeError);
        };
        if name == "ALLOW-OTHER-KEYS" {
            continue;
        }
        if !supplied.insert(name.clone()) {
            continue;
        }
        options = if name == "ARRAY" {
            options.with_array(value != Word::NIL)
        } else if name == "BASE" {
            let ObjectRef::Fixnum(base) = classify_object(ctx, value) else {
                return Err(ObjectError::TypeError);
            };
            let base = u32::try_from(base).map_err(|_| ObjectError::TypeError)?;
            options.try_with_base(base).ok_or(ObjectError::TypeError)?
        } else if name == "CASE" {
            let name = keyword_name(ctx, runtime, value)?;
            let case = if name == "UPCASE" {
                crate::PrintCase::Upcase
            } else if name == "DOWNCASE" {
                crate::PrintCase::Downcase
            } else if name == "CAPITALIZE" {
                crate::PrintCase::Capitalize
            } else {
                return Err(ObjectError::TypeError); // check-added-lines: allow(wildcard) reject unknown CASE values
            };
            options.with_case(case)
        } else if name == "CIRCLE" {
            options.with_circle(value != Word::NIL)
        } else if name == "ESCAPE" {
            options.with_escape(value != Word::NIL)
        } else if name == "GENSYM" {
            options.with_gensym(value != Word::NIL)
        } else if name == "LENGTH" {
            options.with_length(nonnegative_option(ctx, value)?)
        } else if name == "LEVEL" {
            options.with_level(nonnegative_option(ctx, value)?)
        } else if name == "PRETTY" {
            options.with_pretty(value != Word::NIL)
        } else if name == "RADIX" {
            options.with_radix(value != Word::NIL)
        } else if name == "READABLY" {
            options.with_readably(value != Word::NIL)
        } else if allow_other_keys {
            options
        } else {
            return Err(ObjectError::TypeError); // check-added-lines: allow(wildcard) reject unknown WRITE-TO-STRING keys unless allowed
        };
    }
    Ok(options)
}

fn allow_other_keys(
    ctx: &ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
) -> Result<bool, ObjectError> {
    for index in (1..args.len()).step_by(2) {
        let key = args.get(index).ok_or(ObjectError::TypeError)?;
        let value = args.get(index + 1).ok_or(ObjectError::TypeError)?;
        if option_name(ctx, runtime, key)?.as_deref() == Some("ALLOW-OTHER-KEYS") {
            return Ok(value != Word::NIL);
        }
    }
    Ok(false)
}

#[allow(
    clippy::wildcard_enum_match_arm,
    reason = "ObjectRef is non-exhaustive; other object kinds cannot be limits"
)]
fn nonnegative_option(ctx: &ThreadContext, value: Word) -> Result<Option<usize>, ObjectError> {
    if value == Word::NIL {
        return Ok(None);
    }
    match classify_object(ctx, value) {
        ObjectRef::Fixnum(value) => usize::try_from(value)
            .map(Some)
            .map_err(|_| ObjectError::TypeError),
        ObjectRef::Bignum(value) => {
            let value = Bignum::from_word(value);
            if bignum_sign(ctx, value)? {
                return Err(ObjectError::TypeError);
            }
            let mut magnitude = 0_u128;
            for limb in bignum_limbs(ctx, value)?.into_iter().rev() {
                let Some(next) = magnitude
                    .checked_mul(1_u128 << u32::BITS)
                    .and_then(|value| value.checked_add(u128::from(limb)))
                else {
                    return Ok(Some(usize::MAX));
                };
                magnitude = next;
            }
            Ok(Some(usize::try_from(magnitude).unwrap_or(usize::MAX)))
        }
        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) only nonnegative integers are valid limits
    }
}

fn keyword_name(
    ctx: &ThreadContext,
    runtime: &Runtime,
    symbol: Word,
) -> Result<String, ObjectError> {
    option_name(ctx, runtime, symbol)?.ok_or(ObjectError::TypeError)
}

fn option_name(
    ctx: &ThreadContext,
    runtime: &Runtime,
    symbol: Word,
) -> Result<Option<String>, ObjectError> {
    if !matches!(classify_object(ctx, symbol), ObjectRef::Symbol(_)) {
        return Err(ObjectError::TypeError);
    }
    let keyword = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::Layout)?;
    if symbol_package(ctx, symbol)? != keyword {
        return Ok(None);
    }
    let name = symbol_name(ctx, symbol)?;
    let length = ncl_object::string_length(ctx, name)?;
    let mut text = String::with_capacity(length);
    for index in 0..length {
        text.push(ncl_object::string_ref(ctx, name, index)?);
    }
    Ok(Some(text))
}

fn identity_arguments(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    (0..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::TypeError))
        .collect()
}

const fn print_error(error: &PrintError) -> ObjectError {
    match error {
        PrintError::Object(error) => *error,
        PrintError::Sink(_) | PrintError::NotReadable | PrintError::Circularity => {
            ObjectError::Layout
        }
    }
}
