/// Sorting and merging operations for proper lists and simple vectors.

fn list_word(value: ncl_object::List) -> ncl_object::Word {
    match value {
        ncl_object::List::Nil => ncl_object::Word::NIL,
        ncl_object::List::Cons(value) => value.into(),
    }
}

fn sequence_values(
    ctx: &mut ncl_object::ThreadContext,
    value: ncl_object::Sequence,
) -> Result<Vec<ncl_object::Word>, ncl_object::ObjectError> {
    let mut values = Vec::new();
    match value {
        ncl_object::Sequence::List(list) => {
            let mut cursor = list_word(list);
            while cursor != ncl_object::Word::NIL {
                if !cursor.is_cons() {
                    return Err(ncl_object::ObjectError::TypeError);
                }
                values.push(ncl_object::car(ctx, cursor)?);
                cursor = ncl_object::cdr(ctx, cursor)?;
            }
        }
        ncl_object::Sequence::Vector(vector) => {
            let vector: ncl_object::Word = vector.into();
            for index in 0..ncl_object::simple_vector_length(ctx, vector)? {
                values.push(ncl_object::simple_vector_ref(ctx, vector, index)?);
            }
        }
        ncl_object::Sequence::String(string) => {
            let string: ncl_object::Word = string.into();
            for index in 0..ncl_object::string_length(ctx, string)? {
                values.push(ncl_object::Word::character(
                    ncl_object::string_ref(ctx, string, index)? as u32,
                ));
            }
        }
    }
    Ok(values)
}

fn list_from(
    ctx: &mut ncl_object::ThreadContext,
    runtime: &ncl_object::Runtime,
    values: &[ncl_object::Word],
) -> Result<ncl_object::Word, ncl_object::ObjectError> {
    let mut result = ncl_object::Word::NIL;
    for &value in values.iter().rev() {
        result = ncl_object::make_cons(ctx, runtime, value, result)?;
    }
    Ok(result)
}

fn set_sequence_value(
    ctx: &mut ncl_object::ThreadContext,
    value: ncl_object::Sequence,
    index: usize,
    item: ncl_object::Word,
) -> Result<(), ncl_object::ObjectError> {
    match value {
        ncl_object::Sequence::List(list) => {
            let mut cursor = list_word(list);
            for _ in 0..index {
                cursor = ncl_object::cdr(ctx, cursor)?;
            }
            ncl_object::rplaca(ctx, cursor, item)?;
        }
        ncl_object::Sequence::Vector(vector) => {
            ncl_object::simple_vector_set(ctx, vector.into(), index, item)?;
        }
        ncl_object::Sequence::String(string) => {
            let character = char::from_u32((item.bits() >> 4) as u32)
                .ok_or(ncl_object::ObjectError::TypeError)?;
            ncl_object::string_set(ctx, string.into(), index, character)?;
        }
    }
    Ok(())
}

fn call_designator(
    ctx: &mut ncl_object::ThreadContext,
    runtime: &ncl_object::Runtime,
    designator: ncl_object::Word,
    args: &[ncl_object::Word],
) -> Result<ncl_object::Word, ncl_object::ObjectError> {
    let function = match ncl_object::classify_object(ctx, designator) {
        ncl_object::ObjectRef::Function(function) => {
            ncl_object::FunctionObject::try_from(function).map_err(|_| {
                ncl_object::ObjectError::TypeError
            })?
        }
        ncl_object::ObjectRef::Symbol(symbol) => {
            let function = ncl_object::symbol_function(ctx, symbol)?;
            ncl_object::FunctionObject::try_from(function)
                .map_err(|_| ncl_object::ObjectError::TypeError)?
        }
        _ => return Err(ncl_object::ObjectError::TypeError),
    };
    runtime.call_builtin(ctx, function, args)
}

fn keyed(
    ctx: &mut ncl_object::ThreadContext,
    runtime: &ncl_object::Runtime,
    key: Option<ncl_object::Word>,
    value: ncl_object::Word,
) -> Result<ncl_object::Word, ncl_object::ObjectError> {
    key.map_or(Ok(value), |key| call_designator(ctx, runtime, key, &[value]))
}

fn sorted_values(
    ctx: &mut ncl_object::ThreadContext,
    runtime: &ncl_object::Runtime,
    mut values: Vec<ncl_object::Word>,
    predicate: ncl_object::Word,
    key: Option<ncl_object::Word>,
) -> Result<Vec<ncl_object::Word>, ncl_object::ObjectError> {
    let mut keyed_values = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        keyed_values.push((value, keyed(ctx, runtime, key, value)?));
    }

    // Insertion sort is stable and calls the Lisp predicate for every ordering
    // decision. SORT is allowed to be stable, so both entry points can share it.
    for index in 1..keyed_values.len() {
        let item = keyed_values[index];
        let mut position = index;
        while position > 0
            && call_designator(
                ctx,
                runtime,
                predicate,
                &[item.1, keyed_values[position - 1].1],
            )?
                != ncl_object::Word::NIL
        {
            keyed_values[position] = keyed_values[position - 1];
            position -= 1;
        }
        keyed_values[position] = item;
    }
    Ok(keyed_values.into_iter().map(|(value, _)| value).collect())
}

/// Destructively sort a list, vector, or string and return the same sequence.
pub fn sort(
    ctx: &mut ncl_object::ThreadContext,
    runtime: &ncl_object::Runtime,
    value: ncl_object::Sequence,
    predicate: ncl_object::Word,
    key: Option<ncl_object::Word>,
) -> Result<ncl_object::Word, ncl_object::ObjectError> {
    let input = sequence_values(ctx, value)?;
    let values = sorted_values(ctx, runtime, input, predicate, key)?;
    for (index, item) in values.into_iter().enumerate() {
        set_sequence_value(ctx, value, index, item)?;
    }
    Ok(match value {
        ncl_object::Sequence::List(list) => list_word(list),
        ncl_object::Sequence::Vector(vector) => vector.into(),
        ncl_object::Sequence::String(string) => string.into(),
    })
}

/// Destructively stable-sort a list, vector, or string.
pub fn stable_sort(
    ctx: &mut ncl_object::ThreadContext,
    runtime: &ncl_object::Runtime,
    value: ncl_object::Sequence,
    predicate: ncl_object::Word,
    key: Option<ncl_object::Word>,
) -> Result<ncl_object::Word, ncl_object::ObjectError> {
    sort(ctx, runtime, value, predicate, key)
}

/// Compatibility entry point for the sequence-domain builtin adapter.
pub fn sequence_sort(
    ctx: &mut ncl_object::ThreadContext,
    runtime: &ncl_object::Runtime,
    value: ncl_object::Sequence,
    predicate: ncl_object::Word,
    key: Option<ncl_object::Word>,
) -> Result<ncl_object::Word, ncl_object::ObjectError> {
    sort(ctx, runtime, value, predicate, key)
}

/// Compatibility entry point for the sequence-domain builtin adapter.
pub fn sequence_stable_sort(
    ctx: &mut ncl_object::ThreadContext,
    runtime: &ncl_object::Runtime,
    value: ncl_object::Sequence,
    predicate: ncl_object::Word,
    key: Option<ncl_object::Word>,
) -> Result<ncl_object::Word, ncl_object::ObjectError> {
    stable_sort(ctx, runtime, value, predicate, key)
}

/// Merge two sorted sequences into a new list or simple vector.
pub fn merge(
    ctx: &mut ncl_object::ThreadContext,
    runtime: &ncl_object::Runtime,
    result_type: ncl_object::Word,
    first: ncl_object::Sequence,
    second: ncl_object::Sequence,
    predicate: ncl_object::Word,
    key: Option<ncl_object::Word>,
) -> Result<ncl_object::Word, ncl_object::ObjectError> {
    let first = sequence_values(ctx, first)?;
    let second = sequence_values(ctx, second)?;
    let left = first
        .into_iter()
        .map(|value| Ok((value, keyed(ctx, runtime, key, value)?)))
        .collect::<Result<Vec<_>, ncl_object::ObjectError>>()?;
    let right = second
        .into_iter()
        .map(|value| Ok((value, keyed(ctx, runtime, key, value)?)))
        .collect::<Result<Vec<_>, ncl_object::ObjectError>>()?;
    let mut values = Vec::with_capacity(left.len() + right.len());
    let (mut left_index, mut right_index) = (0, 0);
    while left_index < left.len() && right_index < right.len() {
        if call_designator(
            ctx,
            runtime,
            predicate,
            &[right[right_index].1, left[left_index].1],
        )?
            != ncl_object::Word::NIL
        {
            values.push(right[right_index].0);
            right_index += 1;
        } else {
            values.push(left[left_index].0);
            left_index += 1;
        }
    }
    values.extend(left[left_index..].iter().map(|(value, _)| *value));
    values.extend(right[right_index..].iter().map(|(value, _)| *value));

    let name = ncl_object::symbol_name(ctx, result_type)?;
    let name = (0..ncl_object::string_length(ctx, name)?)
        .map(|index| ncl_object::string_ref(ctx, name, index))
        .collect::<Result<String, _>>()?;
    match name.as_str() {
        "LIST" => list_from(ctx, runtime, &values),
        "VECTOR" => ncl_object::make_simple_vector(ctx, runtime, &values),
        _ => Err(ncl_object::ObjectError::TypeError),
    }
}

/// Compatibility entry point for the sequence-domain builtin adapter.
pub fn sequence_merge(
    ctx: &mut ncl_object::ThreadContext,
    runtime: &ncl_object::Runtime,
    result_type: ncl_object::Word,
    first: ncl_object::Sequence,
    second: ncl_object::Sequence,
    predicate: ncl_object::Word,
    key: Option<ncl_object::Word>,
) -> Result<ncl_object::Word, ncl_object::ObjectError> {
    merge(ctx, runtime, result_type, first, second, predicate, key)
}

use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinImplementation, BuiltinName, LambdaList,
    MultipleValues, ObjectError, Parameter, ParameterType, RustBuiltin,
};

const SEQUENCE_PARAMETER: Parameter = Parameter { name: BuiltinName::new("sequence"), ty: ParameterType::Sequence };
const PREDICATE_PARAMETER: Parameter = Parameter { name: BuiltinName::new("predicate"), ty: ParameterType::FunctionDesignator };
const KEY_PARAMETER: Parameter = Parameter { name: BuiltinName::new("key"), ty: ParameterType::FunctionDesignator };

fn pass(args: &BuiltinArgs<'_>) -> Result<Vec<ncl_object::Word>, ObjectError> {
    (0..args.len()).map(|index| args.get(index).ok_or(ObjectError::TypeError)).collect()
}

fn sort_callback(ctx: &mut ncl_object::ThreadContext, runtime: &ncl_object::Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<ncl_object::Word, ObjectError> {
    let values = pass(args)?;
    let sequence = super::sequence_value(ctx, values[0])?;
    let key = values.get(2).copied().filter(|word| *word != ncl_object::Word::NIL);
    sort(ctx, runtime, sequence, values[1], key)
}

fn merge_callback(ctx: &mut ncl_object::ThreadContext, runtime: &ncl_object::Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<ncl_object::Word, ObjectError> {
    let values = pass(args)?;
    let first = super::sequence_value(ctx, values[1])?;
    let second = super::sequence_value(ctx, values[2])?;
    let key = values.get(4).copied().filter(|word| *word != ncl_object::Word::NIL);
    merge(ctx, runtime, values[0], first, second, values[3], key)
}

pub(crate) fn sort_entry(name: &str) -> Option<BuiltinImplementation> {
    let function: RustBuiltin = match name {
        "SORT" | "STABLE-SORT" => sort_callback,
        _ => return None,
    };
    Some(BuiltinImplementation::adapted(
        Builtin { lambda_list: LambdaList::with_rest(&[SEQUENCE_PARAMETER, PREDICATE_PARAMETER], KEY_PARAMETER), convention: BuiltinConvention::Adapted },
        function,
        pass,
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn insertion_sort_preserves_equal_key_order() {
        let mut values = [(2, 'a'), (1, 'b'), (2, 'c'), (1, 'd')];
        for index in 1..values.len() {
            let item = values[index];
            let mut position = index;
            while position > 0 && item.0 < values[position - 1].0 {
                values[position] = values[position - 1];
                position -= 1;
            }
            values[position] = item;
        }
        assert_eq!(values, [(1, 'b'), (1, 'd'), (2, 'a'), (2, 'c')]);
    }
}
