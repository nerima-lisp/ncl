#[derive(Clone, Copy, Default)]
pub(crate) struct SetOptions {
    pub(crate) test: Option<Word>,
    pub(crate) test_not: Option<Word>,
    pub(crate) key: Option<Word>,
}

pub(crate) fn set_options(
    ctx: &ThreadContext,
    args: &[Word],
    start: usize,
) -> Result<SetOptions, LispError> {
    let mut options = SetOptions::default();
    let mut index = start;
    while index < args.len() {
        let keyword = keyword_name(ctx, args[index])?;
        let value = args.get(index + 1).copied().ok_or(LispError::ProgramError(
            ncl_object::ProgramError::OddKeywordArguments,
        ))?;
        match keyword.as_str() {
            "TEST" => options.test = Some(value),
            "TEST-NOT" => options.test_not = Some(value),
            "KEY" => options.key = (value != Word::NIL).then_some(value),
            _ => {
                return Err(LispError::ProgramError(
                    ncl_object::ProgramError::UnknownKeyword,
                ));
            }
        }
        index += 2;
    }
    if options.test.is_some() && options.test_not.is_some() {
        return Err(LispError::TypeError {
            datum: Word::NIL,
            expected: ncl_object::ObjectType::Function,
        });
    }
    Ok(options)
}

fn list_values(ctx: &mut ThreadContext, list: List) -> Result<Vec<Word>, LispError> {
    let mut values = Vec::new();
    let mut cursor = list_word(list);
    while cursor != Word::NIL {
        if !cursor.is_cons() {
            return Err(LispError::TypeError {
                datum: cursor,
                expected: ncl_object::ObjectType::Cons,
            });
        }
        values.push(object_car(ctx, cursor)?);
        cursor = object_cdr(ctx, cursor)?;
    }
    Ok(values)
}

fn predicate2(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    first: Word,
    second: Word,
) -> Result<bool, LispError> {
    let function = match classify_object(ctx, function) {
        ObjectRef::Function(word) => FunctionObject::try_from(word).map_err(LispError::from)?,
        ObjectRef::Symbol(symbol) => {
            FunctionObject::try_from(symbol_function(ctx, symbol)?).map_err(LispError::from)?
        }
        _ => {
            return Err(LispError::TypeError {
                datum: function,
                expected: ncl_object::ObjectType::Function,
            });
        }
    };
    Ok(runtime
        .call_builtin(ctx, function, &[first, second])
        .map_err(LispError::from)?
        != Word::NIL)
}

fn set_member(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    values: &[Word],
    options: SetOptions,
) -> Result<bool, LispError> {
    let item = keyed(ctx, runtime, options.key, item)?;
    for &value in values {
        let value = keyed(ctx, runtime, options.key, value)?;
        let equal = if let Some(test) = options.test {
            predicate2(ctx, runtime, test, item, value)?
        } else if let Some(test_not) = options.test_not {
            !predicate2(ctx, runtime, test_not, item, value)?
        } else {
            item == value
        };
        if equal {
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(Clone, Copy)]
enum SetMode {
    Union,
    Intersection,
    Difference,
    ExclusiveOr,
}

fn combine(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    left: List,
    right: List,
    options: SetOptions,
    mode: SetMode,
) -> Result<Word, LispError> {
    let left = list_values(ctx, left)?;
    let right = list_values(ctx, right)?;
    let mut result = Vec::new();
    match mode {
        SetMode::Union => {
            for &value in left.iter().chain(right.iter()) {
                if !set_member(ctx, runtime, value, &result, options)? {
                    result.push(value);
                }
            }
        }
        SetMode::Intersection => {
            for &value in &left {
                if set_member(ctx, runtime, value, &right, options)?
                    && !set_member(ctx, runtime, value, &result, options)?
                {
                    result.push(value);
                }
            }
        }
        SetMode::Difference => {
            for &value in &left {
                if !set_member(ctx, runtime, value, &right, options)? {
                    result.push(value);
                }
            }
        }
        SetMode::ExclusiveOr => {
            for &value in &left {
                if !set_member(ctx, runtime, value, &right, options)?
                    && !set_member(ctx, runtime, value, &result, options)?
                {
                    result.push(value);
                }
            }
            for &value in &right {
                if !set_member(ctx, runtime, value, &left, options)?
                    && !set_member(ctx, runtime, value, &result, options)?
                {
                    result.push(value);
                }
            }
        }
    }
    list_from(ctx, runtime, &result).map_err(LispError::from)
}

pub(crate) fn adjoin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    list: List,
    options: SetOptions,
) -> Result<Word, LispError> {
    let values = list_values(ctx, list)?;
    if set_member(ctx, runtime, item, &values, options)? {
        return list_from(ctx, runtime, &values).map_err(LispError::from);
    }
    let mut result = vec![item];
    result.extend(values);
    list_from(ctx, runtime, &result).map_err(LispError::from)
}

pub(crate) fn union(ctx: &mut ThreadContext, runtime: &Runtime, left: List, right: List, options: SetOptions) -> Result<Word, LispError> {
    combine(ctx, runtime, left, right, options, SetMode::Union)
}

pub(crate) fn intersection(ctx: &mut ThreadContext, runtime: &Runtime, left: List, right: List, options: SetOptions) -> Result<Word, LispError> {
    combine(ctx, runtime, left, right, options, SetMode::Intersection)
}

pub(crate) fn difference(ctx: &mut ThreadContext, runtime: &Runtime, left: List, right: List, options: SetOptions) -> Result<Word, LispError> {
    combine(ctx, runtime, left, right, options, SetMode::Difference)
}

pub(crate) fn exclusive_or(ctx: &mut ThreadContext, runtime: &Runtime, left: List, right: List, options: SetOptions) -> Result<Word, LispError> {
    combine(ctx, runtime, left, right, options, SetMode::ExclusiveOr)
}

pub(crate) fn subsetp(ctx: &mut ThreadContext, runtime: &Runtime, left: List, right: List, options: SetOptions) -> Result<Word, LispError> {
    let left = list_values(ctx, left)?;
    let right = list_values(ctx, right)?;
    for value in left {
        if !set_member(ctx, runtime, value, &right, options)? {
            return Ok(Word::NIL);
        }
    }
    Ok(Word::TRUE)
}

const fn set_parameter(name: &'static str, ty: ParameterType) -> Parameter {
    Parameter { name: BuiltinName::new(name), ty }
}

const SET_LISTS: &[Parameter] = &[
    set_parameter("first", ParameterType::List),
    set_parameter("second", ParameterType::List),
];
const ADJOIN_ARGS: &[Parameter] = &[
    set_parameter("item", ParameterType::Any),
    set_parameter("list", ParameterType::List),
];
const SET_KEYS: &[Parameter] = &[
    set_parameter("test", ParameterType::FunctionDesignator),
    set_parameter("test-not", ParameterType::FunctionDesignator),
    set_parameter("key", ParameterType::FunctionDesignator),
];

fn set_descriptor(required: &'static [Parameter]) -> Builtin {
    Builtin {
        lambda_list: LambdaList::with_keys(required, SET_KEYS, false),
        convention: BuiltinConvention::Adapted,
    }
}

fn set_args(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    (0..args.len()).map(|i| args.get(i).ok_or(ObjectError::TypeError)).collect()
}

fn set_list(ctx: &mut ThreadContext, value: Word) -> Result<List, ObjectError> {
    <List as ncl_object::FromLispArg>::from_lisp_arg(ctx, value).map_err(|error| {
        ctx.set_pending_lisp_error(error);
        ObjectError::TypeError
    })
}

type SetOperation = fn(&mut ThreadContext, &Runtime, List, List, SetOptions) -> Result<Word, LispError>;

fn set_binary(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    operation: SetOperation,
) -> Result<Word, ObjectError> {
    let values = set_args(args)?;
    let options = set_options(ctx, &values, 2).map_err(|error| {
        ctx.set_pending_lisp_error(error);
        ObjectError::TypeError
    })?;
    let left = set_list(ctx, values[0])?;
    let right = set_list(ctx, values[1])?;
    operation(ctx, runtime, left, right, options).map_err(|error| {
        ctx.set_pending_lisp_error(error);
        ObjectError::TypeError
    })
}

fn adjoin_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    let values = set_args(args)?;
    let options = set_options(ctx, &values, 2).map_err(|error| {
        ctx.set_pending_lisp_error(error);
        ObjectError::TypeError
    })?;
    let list = set_list(ctx, values[1])?;
    adjoin(ctx, runtime, values[0], list, options).map_err(|error| {
        ctx.set_pending_lisp_error(error);
        ObjectError::TypeError
    })
}

fn union_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    set_binary(ctx, runtime, args, union)
}
fn intersection_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    set_binary(ctx, runtime, args, intersection)
}
fn difference_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    set_binary(ctx, runtime, args, difference)
}
fn exclusive_or_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    set_binary(ctx, runtime, args, exclusive_or)
}
fn subsetp_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    set_binary(ctx, runtime, args, subsetp)
}

pub(crate) fn set_entry(name: &str) -> Option<BuiltinImplementation> {
    let (required, implementation): (&'static [Parameter], RustBuiltin) = match name {
        "ADJOIN" => (ADJOIN_ARGS, adjoin_builtin),
        "UNION" | "NUNION" => (SET_LISTS, union_builtin),
        "INTERSECTION" | "NINTERSECTION" => (SET_LISTS, intersection_builtin),
        "SET-DIFFERENCE" | "NSET-DIFFERENCE" => (SET_LISTS, difference_builtin),
        "SET-EXCLUSIVE-OR" | "NSET-EXCLUSIVE-OR" => (SET_LISTS, exclusive_or_builtin),
        "SUBSETP" => (SET_LISTS, subsetp_builtin),
        _ => return None,
    };
    Some(BuiltinImplementation::adapted(
        set_descriptor(required),
        implementation,
        set_args,
    ))
}

#[cfg(test)]
mod set_tests {
    use super::set_entry;

    #[test]
    fn all_set_operations_have_typed_implementations() {
        for name in [
            "ADJOIN",
            "UNION",
            "NUNION",
            "INTERSECTION",
            "NINTERSECTION",
            "SET-DIFFERENCE",
            "NSET-DIFFERENCE",
            "SET-EXCLUSIVE-OR",
            "NSET-EXCLUSIVE-OR",
            "SUBSETP",
        ] {
            assert!(set_entry(name).is_some(), "missing implementation for {name}");
        }
        assert!(set_entry("UNRELATED-SEQUENCE-FUNCTION").is_none());
    }
}
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinImplementation, BuiltinName, LambdaList,
    MultipleValues, Parameter, ParameterType, RustBuiltin,
};
