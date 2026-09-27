//! The compiler's keyword-argument helper builtins.
//!
//! These helpers are object-runtime operations rather than user-facing
//! Common Lisp functions; [`Runtime::register_keyword_builtins`] registers
//! them in `NCL-EXT` so the native runtime and the safe builtin caller use
//! the same implementation.
//!
//! [`Runtime::register_keyword_builtins`]: crate::Runtime::register_keyword_builtins

use crate::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinName, LambdaList, LispError,
    ObjectError, ObjectRef, Parameter, ParameterType, ProgramError, Runtime, ThreadContext, Word,
    car, cdr, classify_object, make_cons, string_length, string_ref, symbol_name, symbol_package,
    with_root, with_roots,
};

const KEYWORD_LIST: Parameter = Parameter {
    name: BuiltinName::new("LIST"),
    ty: ParameterType::Any,
};
const KEYWORD: Parameter = Parameter {
    name: BuiltinName::new("KEYWORD"),
    ty: ParameterType::Any,
};
const ALLOW_OTHER_KEYS: Parameter = Parameter {
    name: BuiltinName::new("ALLOW-OTHER-KEYS"),
    ty: ParameterType::Any,
};
const ALLOWED_KEYWORD: Parameter = Parameter {
    name: BuiltinName::new("ALLOWED-KEYWORD"),
    ty: ParameterType::Any,
};
pub const CHECK_KEYWORDS_DESCRIPTOR: Builtin = Builtin {
    lambda_list: LambdaList::new(
        &[KEYWORD_LIST, ALLOW_OTHER_KEYS],
        &[],
        Some(ALLOWED_KEYWORD),
        &[],
        false,
    ),
    convention: BuiltinConvention::Direct(Arity::exact(2)),
};
const REST_LIST_REQUIRED: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("ARGC"),
        ty: ParameterType::Fixnum,
    },
    Parameter {
        name: BuiltinName::new("START"),
        ty: ParameterType::Fixnum,
    },
];
const REST_LIST_VALUE: Parameter = Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ParameterType::Any,
};
pub const MAKE_REST_LIST_DESCRIPTOR: Builtin = Builtin {
    lambda_list: LambdaList::with_rest(REST_LIST_REQUIRED, REST_LIST_VALUE),
    convention: BuiltinConvention::Adapted,
};
pub const KEYWORD_VALUE_DESCRIPTOR: Builtin = Builtin {
    lambda_list: LambdaList::new(&[KEYWORD_LIST, KEYWORD], &[], None, &[], false),
    convention: BuiltinConvention::Direct(Arity::exact(2)),
};

pub fn make_rest_list_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    builtin_args: &BuiltinArgs<'_>,
    _values: &mut crate::MultipleValues,
) -> Result<Word, ObjectError> {
    let argc = builtin_args
        .required(0)?
        .as_fixnum()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(ObjectError::TypeError)?;
    let start = builtin_args
        .required(1)?
        .as_fixnum()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(ObjectError::TypeError)?;
    let values = builtin_args
        .as_slice()
        .get(2..)
        .ok_or(ObjectError::TypeError)?;
    if start > argc || argc > values.len() {
        return Err(ObjectError::TypeError);
    }
    // check-added-lines: allow(index) bounded above by the `argc > values.len()` guard.
    with_roots(ctx, &values[..argc], |ctx, values| {
        let mut list = Word::NIL;
        with_root(ctx, &mut list, |ctx, list| {
            let mut list_word = *list;
            // check-added-lines: allow(index) bounded above by the `start > argc` guard.
            for value in values[start..].iter().rev() {
                list_word = make_cons(ctx, runtime, **value, list_word)?;
            }
            Ok(list_word)
        })
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KeywordEntriesError {
    Type,
    Odd,
}

impl KeywordEntriesError {
    const fn object_error() -> ObjectError {
        ObjectError::TypeError
    }
}

fn keyword_entries(
    ctx: &ThreadContext,
    list: Word,
) -> Result<Vec<(Word, Word)>, KeywordEntriesError> {
    let mut entries = Vec::new();
    let mut cursor = list;
    while cursor != Word::NIL {
        let key = car(ctx, cursor).map_err(|_| KeywordEntriesError::Type)?;
        let tail = cdr(ctx, cursor).map_err(|_| KeywordEntriesError::Type)?;
        if tail == Word::NIL {
            return Err(KeywordEntriesError::Odd);
        }
        let value = car(ctx, tail).map_err(|_| KeywordEntriesError::Type)?;
        let next = cdr(ctx, tail).map_err(|_| KeywordEntriesError::Type)?;
        if matches!(classify_object(ctx, key), ObjectRef::Symbol(_)) {
            entries.push((key, value));
        } else {
            return Err(KeywordEntriesError::Type);
        }
        cursor = next;
    }
    Ok(entries)
}

fn is_allow_other_keys(
    ctx: &ThreadContext,
    runtime: &Runtime,
    symbol: Word,
) -> Result<bool, ObjectError> {
    let Some(keyword_package) = runtime.find_package(ctx, "KEYWORD") else {
        return Ok(false);
    };
    if symbol_package(ctx, symbol)? != keyword_package {
        return Ok(false);
    }
    let name = symbol_name(ctx, symbol)?;
    if string_length(ctx, name)? != "ALLOW-OTHER-KEYS".len() {
        return Ok(false);
    }
    Ok("ALLOW-OTHER-KEYS"
        .chars()
        .enumerate()
        .all(|(index, expected)| string_ref(ctx, name, index) == Ok(expected)))
}

pub fn check_keywords_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut crate::MultipleValues,
) -> Result<Word, ObjectError> {
    let list = args.required(0)?;
    let lambda_allows_other_keys = args.required(1)? != Word::NIL;
    let entries = match keyword_entries(ctx, list) {
        Ok(entries) => entries,
        Err(KeywordEntriesError::Odd) => {
            ctx.set_pending_lisp_error(LispError::ProgramError(ProgramError::OddKeywordArguments));
            return Err(ObjectError::TypeError);
        }
        Err(_error) => return Err(KeywordEntriesError::object_error()),
    };
    let mut call_allows_other_keys = false;
    for (keyword, value) in &entries {
        if is_allow_other_keys(ctx, runtime, *keyword)? && *value != Word::NIL {
            call_allows_other_keys = true;
        }
    }
    if !lambda_allows_other_keys && !call_allows_other_keys {
        // check-added-lines: allow(index) the lambda list requires 2 args before any allowed keyword.
        let allowed = &args.as_slice()[2..];
        for (keyword, _) in &entries {
            if !is_allow_other_keys(ctx, runtime, *keyword)? && !allowed.contains(keyword) {
                ctx.set_pending_lisp_error(LispError::ProgramError(ProgramError::UnknownKeyword));
                return Err(ObjectError::TypeError);
            }
        }
    }
    Ok(Word::NIL)
}

pub fn keyword_value_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut crate::MultipleValues,
) -> Result<Word, ObjectError> {
    let list = args.required(0)?;
    let keyword = args.required(1)?;
    keyword_entries(ctx, list)
        .map_err(|_| KeywordEntriesError::object_error())?
        .into_iter()
        .find(|(candidate, _)| *candidate == keyword)
        .map_or(Ok(Word::NIL), |(_, value)| Ok(value))
}

pub fn keyword_supplied_p_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut crate::MultipleValues,
) -> Result<Word, ObjectError> {
    let list = args.required(0)?;
    let keyword = args.required(1)?;
    Ok(
        if keyword_entries(ctx, list)
            .map_err(|_| KeywordEntriesError::object_error())?
            .into_iter()
            .any(|(candidate, _)| candidate == keyword)
        {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}
