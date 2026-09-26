use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinFunctionCaller, BuiltinImplementation,
    BuiltinName, Cons, FunctionArguments, FunctionCaller, LambdaList, LispError, List,
    MultipleValues, ObjectError, ObjectRef, Parameter, ParameterType, Runtime, RustBuiltin,
    Sequence, SimpleVector, StringObject, ThreadContext, Word, car as object_car,
    cdr as object_cdr, classify_object, make_cons, pop_root, push_root, rplaca as object_rplaca,
    string_length, string_ref, symbol_name,
};

fn list_word(value: List) -> Word {
    match value {
        List::Nil => Word::NIL,
        List::Cons(value) => value.into(),
    }
}

fn list_from(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    let mut rooted_values = values.to_vec();
    let value_tokens = rooted_values
        .iter_mut()
        .map(|value| push_root(ctx, value))
        .collect::<Vec<_>>();
    let mut result = Word::NIL;
    let result_token = push_root(ctx, &mut result);
    let result_value = rooted_values
        .iter()
        .rev()
        .try_fold(Word::NIL, |tail, &value| {
            result = tail;
            let next = make_cons(ctx, runtime, value, result)?;
            result = next;
            Ok(next)
        });
    let result_root_error = (!pop_root(ctx, result_token)).then_some(ObjectError::Layout);
    let value_root_error = value_tokens
        .into_iter()
        .rev()
        .find_map(|token| (!pop_root(ctx, token)).then_some(ObjectError::Layout));
    if let Some(error) = value_root_error.or(result_root_error) {
        return Err(error);
    }
    result_value
}

fn sequence_value(ctx: &mut ThreadContext, value: Word) -> Result<Sequence, ObjectError> {
    match classify_object(ctx, value) {
        ObjectRef::Symbol(_) if value == Word::NIL => Ok(Sequence::List(List::Nil)),
        ObjectRef::Cons(_) => Ok(Sequence::List(List::Cons(Cons::from_word(value)))),
        ObjectRef::String(_) => Ok(Sequence::String(StringObject::from_word(value))),
        ObjectRef::SimpleVector(_) => Ok(Sequence::Vector(SimpleVector::from_word(value))),
        _ => Err(ObjectError::TypeError),
    }
}

fn seq_values(ctx: &mut ThreadContext, value: Sequence) -> Result<Vec<Word>, ObjectError> {
    match value {
        Sequence::List(list) => {
            let mut values = Vec::new();
            let mut cursor = list_word(list);
            while cursor != Word::NIL {
                if !cursor.is_cons() {
                    return Err(ObjectError::TypeError);
                }
                values.push(object_car(ctx, cursor)?);
                cursor = object_cdr(ctx, cursor)?;
            }
            Ok(values)
        }
        Sequence::String(string) => {
            let string: Word = string.into();
            (0..string_length(ctx, string)?)
                .map(|index| Ok(Word::character(string_ref(ctx, string, index)? as u32)))
                .collect()
        }
        Sequence::Vector(vector) => {
            let vector: Word = vector.into();
            (0..ncl_object::simple_vector_length(ctx, vector)?)
                .map(|index| ncl_object::simple_vector_ref(ctx, vector, index))
                .collect()
        }
    }
}

fn sequence_length(ctx: &mut ThreadContext, value: Sequence) -> Result<usize, ObjectError> {
    Ok(seq_values(ctx, value)?.len())
}

fn call_designator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    designator: Word,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let mut designator_word = designator;
    let mut rooted_args = args.to_vec();
    let designator_token = push_root(ctx, &mut designator_word);
    let argument_tokens = rooted_args
        .iter_mut()
        .map(|argument| push_root(ctx, argument))
        .collect::<Vec<_>>();
    let result = FunctionDesignator::try_from_word(ctx, designator_word).and_then(|designator| {
        let mut caller = BuiltinFunctionCaller;
        let mut values = MultipleValues::new();
        caller.call_function(
            ctx,
            runtime,
            designator,
            FunctionArguments::new(&rooted_args),
            &mut values,
        )
    });
    let argument_root_error = argument_tokens
        .into_iter()
        .rev()
        .find_map(|token| (!pop_root(ctx, token)).then_some(ObjectError::Layout));
    let designator_root_error = (!pop_root(ctx, designator_token)).then_some(ObjectError::Layout);
    if let Some(error) = argument_root_error.or(designator_root_error) {
        return Err(error);
    }
    result
}

const fn parameter(name: &'static str, ty: ParameterType) -> Parameter {
    Parameter {
        name: BuiltinName::new(name),
        ty,
    }
}

const MAP_REQUIRED: &[Parameter] = &[
    parameter("result-type", ParameterType::Any),
    parameter("function", ParameterType::FunctionDesignator),
];
const MAP_INTO_REQUIRED: &[Parameter] = &[
    parameter("result", ParameterType::Sequence),
    parameter("function", ParameterType::FunctionDesignator),
];
const LIST_MAP_REQUIRED: &[Parameter] = &[
    parameter("function", ParameterType::FunctionDesignator),
    parameter("list", ParameterType::List),
];
const SEQUENCES: Parameter = parameter("sequences", ParameterType::Sequence);
const LISTS: Parameter = parameter("lists", ParameterType::List);

fn pass_arguments(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    (0..args.len())
        .map(|index| args.get(index).ok_or(ObjectError::TypeError))
        .collect()
}

fn call_designator_map(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    args: &[Word],
) -> Result<Word, ObjectError> {
    call_designator(ctx, runtime, function, args).map_err(|error| {
        ctx.set_pending_lisp_error(LispError::Object(error));
        ObjectError::TypeError
    })
}

fn sequence_words(
    ctx: &mut ThreadContext,
    args: &[Word],
) -> Result<(Vec<Vec<Word>>, usize), ObjectError> {
    if args.is_empty() {
        return Err(ObjectError::TypeError);
    }
    let mut values = Vec::with_capacity(args.len());
    let mut length = usize::MAX;
    for &word in args {
        let sequence = sequence_value(ctx, word)?;
        let sequence_values = seq_values(ctx, sequence)?;
        length = length.min(sequence_values.len());
        values.push(sequence_values);
    }
    Ok((values, length))
}

fn result_type_name(ctx: &mut ThreadContext, word: Word) -> Result<String, ObjectError> {
    let ObjectRef::Symbol(_) = classify_object(ctx, word) else {
        return Err(ObjectError::TypeError);
    };
    let name = symbol_name(ctx, word)?;
    (0..string_length(ctx, name)?)
        .map(|index| string_ref(ctx, name, index))
        .collect()
}

fn map_result(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    result_type: Word,
    values: &[Word],
) -> Result<Word, ObjectError> {
    match result_type_name(ctx, result_type)?.as_str() {
        "NIL" => Ok(Word::NIL),
        "LIST" => list_from(ctx, runtime, values),
        "VECTOR" => ncl_object::make_simple_vector(ctx, runtime, values),
        "STRING" => {
            let chars = values
                .iter()
                .map(|value| {
                    char::from_u32((value.bits() >> 4) as u32).ok_or(ObjectError::TypeError)
                })
                .collect::<Result<Vec<_>, _>>()?;
            ncl_object::make_string(ctx, runtime, &chars)
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn map_sequence(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    result_type: Word,
    function: Word,
    sequences: &[Word],
) -> Result<Word, ObjectError> {
    let (values, length) = sequence_words(ctx, sequences)?;
    let mut result = Vec::with_capacity(length);
    for index in 0..length {
        let arguments = values
            .iter()
            .map(|sequence| sequence[index])
            .collect::<Vec<_>>();
        result.push(call_designator_map(ctx, runtime, function, &arguments)?);
    }
    map_result(ctx, runtime, result_type, &result)
}

fn sequence_set(
    ctx: &mut ThreadContext,
    sequence: Sequence,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    match sequence {
        Sequence::List(list) => {
            let mut cursor = list_word(list);
            for _ in 0..index {
                cursor = object_cdr(ctx, cursor)?;
            }
            object_rplaca(ctx, cursor, value)?;
        }
        Sequence::String(string) => {
            let character =
                char::from_u32((value.bits() >> 4) as u32).ok_or(ObjectError::TypeError)?;
            ncl_object::string_set(ctx, string.into(), index, character)?;
        }
        Sequence::Vector(vector) => {
            ncl_object::simple_vector_set(ctx, vector.into(), index, value)?;
        }
    }
    Ok(())
}

fn map_into(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    result: Word,
    function: Word,
    sources: &[Word],
) -> Result<Word, ObjectError> {
    let destination = sequence_value(ctx, result)?;
    let destination_length = sequence_length(ctx, destination)?;
    let (values, source_length) = sequence_words(ctx, sources)?;
    let length = destination_length.min(source_length);
    for index in 0..length {
        let arguments = values
            .iter()
            .map(|source| source[index])
            .collect::<Vec<_>>();
        let value = call_designator_map(ctx, runtime, function, &arguments)?;
        sequence_set(ctx, destination, index, value)?;
    }
    Ok(result)
}

fn list_map(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    lists: &[Word],
    tails: bool,
    concatenate: bool,
) -> Result<Word, ObjectError> {
    if lists.is_empty() {
        return Err(ObjectError::TypeError);
    }
    let mut cursors = Vec::with_capacity(lists.len());
    for &word in lists {
        let Sequence::List(list) = sequence_value(ctx, word)? else {
            return Err(ObjectError::TypeError);
        };
        cursors.push(list_word(list));
    }

    let mut mapped = Vec::new();
    let mut concatenated = Vec::new();
    loop {
        if cursors.iter().any(|cursor| *cursor == Word::NIL) {
            break;
        }
        if cursors.iter().any(|cursor| !cursor.is_cons()) {
            return Err(ObjectError::TypeError);
        }
        let arguments = if tails {
            cursors.clone()
        } else {
            cursors
                .iter()
                .map(|cursor| object_car(ctx, *cursor))
                .collect::<Result<Vec<_>, _>>()?
        };
        let value = call_designator_map(ctx, runtime, function, &arguments)?;
        if concatenate {
            let Sequence::List(list) = sequence_value(ctx, value)? else {
                return Err(ObjectError::TypeError);
            };
            let mut cursor = list_word(list);
            while cursor != Word::NIL {
                if !cursor.is_cons() {
                    return Err(ObjectError::TypeError);
                }
                concatenated.push(object_car(ctx, cursor)?);
                cursor = object_cdr(ctx, cursor)?;
            }
        } else {
            mapped.push(value);
        }
        for cursor in &mut cursors {
            *cursor = object_cdr(ctx, *cursor)?;
        }
    }
    if concatenate {
        list_from(ctx, runtime, &concatenated)
    } else {
        list_from(ctx, runtime, &mapped)
    }
}

fn map_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let values = pass_arguments(args)?;
    if values.len() < 3 {
        return Err(ObjectError::TypeError);
    }
    map_sequence(ctx, runtime, values[0], values[1], &values[2..])
}

fn map_into_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let values = pass_arguments(args)?;
    if values.len() < 3 {
        return Err(ObjectError::TypeError);
    }
    map_into(ctx, runtime, values[0], values[1], &values[2..])
}

fn list_map_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    tails: bool,
    concatenate: bool,
) -> Result<Word, ObjectError> {
    let values = pass_arguments(args)?;
    if values.len() < 2 {
        return Err(ObjectError::TypeError);
    }
    list_map(ctx, runtime, values[0], &values[1..], tails, concatenate)
}

fn mapc_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let values = pass_arguments(args)?;
    if values.len() < 2 {
        return Err(ObjectError::TypeError);
    }
    let first = values[1];
    list_map(ctx, runtime, values[0], &values[1..], false, false).map(|_| first)
}

fn mapl_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let values = pass_arguments(args)?;
    if values.len() < 2 {
        return Err(ObjectError::TypeError);
    }
    let first = values[1];
    list_map(ctx, runtime, values[0], &values[1..], true, false).map(|_| first)
}

fn mapcar_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    list_map_callback(ctx, runtime, args, false, false)
}

fn mapcan_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    list_map_callback(ctx, runtime, args, false, true)
}

fn maplist_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    list_map_callback(ctx, runtime, args, true, false)
}

fn mapcon_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    list_map_callback(ctx, runtime, args, true, true)
}

fn implementation(descriptor: Builtin, function: RustBuiltin) -> BuiltinImplementation {
    BuiltinImplementation::adapted(descriptor, function, pass_arguments)
}

pub(crate) fn map_entry(name: &str) -> Option<BuiltinImplementation> {
    let list_map_descriptor = || Builtin {
        lambda_list: LambdaList::with_rest(LIST_MAP_REQUIRED, LISTS),
        convention: BuiltinConvention::Adapted,
    };
    let implementation = match name {
        "MAP" => implementation(
            Builtin {
                lambda_list: LambdaList::with_rest(MAP_REQUIRED, SEQUENCES),
                convention: BuiltinConvention::Adapted,
            },
            map_callback,
        ),
        "MAP-INTO" => implementation(
            Builtin {
                lambda_list: LambdaList::with_rest(MAP_INTO_REQUIRED, SEQUENCES),
                convention: BuiltinConvention::Adapted,
            },
            map_into_callback,
        ),
        "MAPC" => implementation(list_map_descriptor(), mapc_callback),
        "MAPCAR" => implementation(list_map_descriptor(), mapcar_callback),
        "MAPCAN" => implementation(list_map_descriptor(), mapcan_callback),
        "MAPL" => implementation(list_map_descriptor(), mapl_callback),
        "MAPLIST" => implementation(list_map_descriptor(), maplist_callback),
        "MAPCON" => implementation(list_map_descriptor(), mapcon_callback),
        _ => return None,
    };
    Some(implementation)
}

#[cfg(test)]
mod map_tests {
    use super::map_entry;

    #[test]
    fn all_map_builtins_have_typed_descriptors() {
        for name in [
            "MAP", "MAP-INTO", "MAPC", "MAPCAR", "MAPCAN", "MAPL", "MAPLIST", "MAPCON",
        ] {
            let implementation = map_entry(name);
            assert!(implementation.is_some(), "map builtin must be registered");
            if let Some(implementation) = implementation {
                assert!(implementation.descriptor.lambda_list.min_arity() >= 2);
                assert!(implementation.descriptor.lambda_list.rest.is_some());
            }
        }
    }
}
