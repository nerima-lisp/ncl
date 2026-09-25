fn sort_key(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    key: Option<Word>,
    value: Word,
) -> Result<Word, LispError> {
    keyed(ctx, runtime, key, value)
}

fn sort_values(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: Vec<Word>,
    predicate: Word,
    key: Option<Word>,
) -> Result<Vec<Word>, LispError> {
    let mut keyed_values = values
        .into_iter()
        .map(|value| Ok((value, sort_key(ctx, runtime, key, value)?)))
        .collect::<Result<Vec<_>, LispError>>()?;

    // Insertion sort keeps equal elements in their original order.  This is
    // also a valid implementation of SORT, whose stability is unspecified.
    for index in 1..keyed_values.len() {
        let value = keyed_values[index];
        let mut position = index;
        while position > 0
            && call_designator(
                ctx,
                runtime,
                predicate,
                &[value.1, keyed_values[position - 1].1],
            )? != Word::NIL
        {
            keyed_values[position] = keyed_values[position - 1];
            position -= 1;
        }
        keyed_values[position] = value;
    }
    Ok(keyed_values.into_iter().map(|(value, _)| value).collect())
}

fn replace_sequence(
    ctx: &mut ThreadContext,
    value: Sequence,
    values: &[Word],
) -> Result<Word, LispError> {
    for (index, &item) in values.iter().enumerate() {
        match value {
            Sequence::List(list) => {
                let mut cursor = list_word(list);
                for _ in 0..index {
                    cursor = object_cdr(ctx, cursor)?;
                }
                object_rplaca(ctx, cursor, item)?;
            }
            Sequence::String(string) => string_set(
                ctx,
                string.into(),
                index,
                char::from_u32((item.bits() >> 4) as u32).ok_or(LispError::TypeError {
                    datum: item,
                    expected: ncl_object::ObjectType::Character,
                })?,
            )?,
            Sequence::Vector(vector) => simple_vector_set(ctx, vector.into(), index, item)?,
        }
    }
    Ok(match value {
        Sequence::List(list) => list_word(list),
        Sequence::String(string) => string.into(),
        Sequence::Vector(vector) => vector.into(),
    })
}

pub fn sort(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
    predicate: Word,
    key: Option<Word>,
) -> Result<Word, LispError> {
    let values = seq_values(ctx, value).map_err(LispError::from)?;
    let values = sort_values(ctx, runtime, values, predicate, key)?;
    replace_sequence(ctx, value, &values)
}

pub fn stable_sort(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
    predicate: Word,
    key: Option<Word>,
) -> Result<Word, LispError> {
    sort(ctx, runtime, value, predicate, key)
}

pub fn sequence_sort(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
    predicate: Word,
    key: Option<Word>,
) -> Result<Word, LispError> {
    sort(ctx, runtime, value, predicate, key)
}

pub fn sequence_stable_sort(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Sequence,
    predicate: Word,
    key: Option<Word>,
) -> Result<Word, LispError> {
    stable_sort(ctx, runtime, value, predicate, key)
}

fn result_sequence(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    result_type: Word,
    values: &[Word],
) -> Result<Word, LispError> {
    let name = keyword_name(ctx, result_type)?;
    match name.as_str() {
        "LIST" => list_from(ctx, runtime, values).map_err(LispError::from),
        "VECTOR" => make_simple_vector(ctx, runtime, values).map_err(LispError::from),
        "STRING" => {
            let chars = values
                .iter()
                .map(|value| {
                    char::from_u32((value.bits() >> 4) as u32).ok_or(LispError::TypeError {
                        datum: *value,
                        expected: ncl_object::ObjectType::Character,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            make_string(ctx, runtime, &chars).map_err(LispError::from)
        }
        _ => Err(LispError::TypeError {
            datum: result_type,
            expected: ncl_object::ObjectType::Symbol,
        }),
    }
}

pub fn merge(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    result_type: Word,
    first: Sequence,
    second: Sequence,
    predicate: Word,
    key: Option<Word>,
) -> Result<Word, LispError> {
    let first = seq_values(ctx, first).map_err(LispError::from)?;
    let second = seq_values(ctx, second).map_err(LispError::from)?;
    let first = first
        .into_iter()
        .map(|value| Ok((value, sort_key(ctx, runtime, key, value)?)))
        .collect::<Result<Vec<_>, LispError>>()?;
    let second = second
        .into_iter()
        .map(|value| Ok((value, sort_key(ctx, runtime, key, value)?)))
        .collect::<Result<Vec<_>, LispError>>()?;
    let mut left = 0;
    let mut right = 0;
    let mut values = Vec::with_capacity(first.len() + second.len());
    while left < first.len() && right < second.len() {
        if call_designator(ctx, runtime, predicate, &[second[right].1, first[left].1])? != Word::NIL
        {
            values.push(second[right].0);
            right += 1;
        } else {
            values.push(first[left].0);
            left += 1;
        }
    }
    values.extend(first[left..].iter().map(|(value, _)| *value));
    values.extend(second[right..].iter().map(|(value, _)| *value));
    result_sequence(ctx, runtime, result_type, &values)
}

pub fn sequence_merge(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    result_type: Word,
    first: Sequence,
    second: Sequence,
    predicate: Word,
    key: Option<Word>,
) -> Result<Word, LispError> {
    merge(ctx, runtime, result_type, first, second, predicate, key)
}

#[cfg(test)]
mod tests {
    fn stable_insertion_sort(values: &mut [(i32, char)]) {
        for index in 1..values.len() {
            let value = values[index];
            let mut position = index;
            while position > 0 && value.0 < values[position - 1].0 {
                values[position] = values[position - 1];
                position -= 1;
            }
            values[position] = value;
        }
    }

    #[test]
    fn stable_order_is_preserved_for_equal_keys() {
        let mut values = [(2, 'a'), (1, 'b'), (2, 'c'), (1, 'd')];
        stable_insertion_sort(&mut values);
        assert_eq!(values, [(1, 'b'), (1, 'd'), (2, 'a'), (2, 'c')]);
    }

    #[test]
    fn merge_prefers_left_sequence_when_keys_are_equal() {
        let left = [(1, 'a'), (3, 'c')];
        let right = [(1, 'b'), (2, 'd')];
        let mut output = Vec::new();
        let (mut l, mut r) = (0, 0);
        while l < left.len() && r < right.len() {
            if right[r].0 < left[l].0 {
                output.push(right[r]);
                r += 1;
            } else {
                output.push(left[l]);
                l += 1;
            }
        }
        output.extend_from_slice(&left[l..]);
        output.extend_from_slice(&right[r..]);
        assert_eq!(output, [(1, 'a'), (1, 'b'), (2, 'd'), (3, 'c')]);
    }
}
