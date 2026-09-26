#![allow(dead_code, clippy::redundant_pub_crate)]

#[path = "order_sets_assoc.rs"]
mod assoc;
#[allow(unused_imports)]
pub use assoc::{assoc, member, rassoc};
#[path = "order_sets_extra.rs"]
mod extra;
#[allow(unused_imports)]
pub use extra::{adjoin, intersection, set_difference, set_exclusive_or, subsetp};
use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    BuiltinFunctionCaller, FunctionArguments, FunctionCaller, MultipleValues, ObjectError,
    ObjectRef, Runtime, ThreadContext, Word, car, cdr, classify_object, pop_root, push_root,
    rplaca, simple_vector_length, simple_vector_ref, simple_vector_set,
};
pub(super) fn list_from(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    let mut rooted = values.to_vec();
    let roots = rooted
        .iter_mut()
        .map(|value| push_root(ctx, value))
        .collect::<Vec<_>>();
    let mut result = Word::NIL;
    let result_root = push_root(ctx, &mut result);
    for &value in rooted.iter().rev() {
        result = ncl_object::make_cons(ctx, runtime, value, result)?;
    }
    let valid =
        pop_root(ctx, result_root) && roots.into_iter().rev().all(|root| pop_root(ctx, root));
    if valid {
        Ok(result)
    } else {
        Err(ObjectError::Layout)
    }
}
pub(super) fn rooted_nested<T>(
    ctx: &mut ThreadContext,
    words: &mut [Vec<Word>], // check-added-lines: allow(index) slice type
    f: impl FnOnce(&mut ThreadContext, &[Vec<Word>]) -> T,
) -> T {
    let mut tokens = Vec::new();
    for row in words.iter_mut() {
        for word in row {
            tokens.push(push_root(ctx, word));
        }
    }
    let result = f(ctx, words);
    tokens.into_iter().rev().all(|token| pop_root(ctx, token));
    result
}
#[derive(Clone, Copy)]
pub struct Options {
    pub key: Word,
    pub test: Word,
    pub test_not: Word,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            key: Word::NIL,
            test: Word::NIL,
            test_not: Word::NIL,
        }
    }
}
pub(super) fn with_options<T>(
    ctx: &mut ThreadContext,
    opts: Options,
    f: impl FnOnce(&mut ThreadContext, Options) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let rooted = [opts.key, opts.test, opts.test_not];
    ncl_object::with_roots(ctx, &rooted, |ctx, rooted| {
        f(
            ctx,
            Options {
                key: **rooted.first().ok_or(ObjectError::Layout)?,
                test: **rooted.get(1).ok_or(ObjectError::Layout)?,
                test_not: **rooted.get(2).ok_or(ObjectError::Layout)?,
            },
        )
    })
}
fn symbol_name(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    let ObjectRef::Symbol(symbol) = classify_object(ctx, word) else {
        return Err(ObjectError::TypeError);
    };
    let name = ncl_object::symbol_name(ctx, symbol)?;
    (0..ncl_object::string_length(ctx, name)?)
        .map(|i| ncl_object::string_ref(ctx, name, i))
        .collect()
}
fn options(
    ctx: &ThreadContext,
    args: &[Word],
    required: usize,
) -> Result<(Vec<Word>, Options), ObjectError> {
    let mut positional = Vec::new();
    let mut out = Options {
        key: Word::NIL,
        test: Word::NIL,
        test_not: Word::NIL,
    };
    let mut i = 0;
    while i < args.len() {
        let name = if matches!(
            classify_object(ctx, *args.get(i).ok_or(ObjectError::Layout)?),
            ObjectRef::Symbol(_)
        ) {
            Some(symbol_name(ctx, *args.get(i).ok_or(ObjectError::Layout)?)?)
        } else {
            None
        };
        let keyword = name.as_deref().map(str::to_ascii_uppercase);
        if matches!(keyword.as_deref(), Some("KEY" | "TEST" | "TEST-NOT")) {
            let value = *args.get(i + 1).ok_or(ObjectError::TypeError)?;
            if keyword.as_deref() == Some("KEY") {
                out.key = value;
            } else if keyword.as_deref() == Some("TEST") {
                out.test = value;
            } else if keyword.as_deref() == Some("TEST-NOT") {
                out.test_not = value;
            }
            i += 2;
        } else {
            positional.push(*args.get(i).ok_or(ObjectError::Layout)?);
            i += 1;
        }
    }
    if positional.len() < required || (out.test != Word::NIL && out.test_not != Word::NIL) {
        return Err(ObjectError::TypeError);
    }
    Ok((positional, out))
}
pub fn parse_options(
    ctx: &ThreadContext,
    args: &[Word],
    required: usize,
) -> Result<(Vec<Word>, Options), ObjectError> {
    options(ctx, args, required)
}
pub(super) fn sequence_values(
    ctx: &mut ThreadContext,
    sequence: Word,
) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    if sequence == Word::NIL {
        return Ok(result);
    }
    if sequence.is_cons() {
        let mut cursor = sequence;
        while cursor != Word::NIL {
            if !cursor.is_cons() {
                return Err(ObjectError::TypeError);
            }
            result.push(car(ctx, cursor)?);
            cursor = cdr(ctx, cursor)?;
        }
        return Ok(result);
    }
    if let ObjectRef::SimpleVector(vector) = classify_object(ctx, sequence) {
        let vector: Word = ncl_object::SimpleVector::from_word(vector).into();
        for i in 0..simple_vector_length(ctx, vector)? {
            result.push(simple_vector_ref(ctx, vector, i)?);
        }
        Ok(result)
    } else {
        Err(ObjectError::TypeError)
    }
}
fn set_sequence_value(
    ctx: &mut ThreadContext,
    sequence: Word,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    if sequence.is_cons() {
        let mut cursor = sequence;
        for _ in 0..index {
            cursor = cdr(ctx, cursor)?;
        }
        rplaca(ctx, cursor, value)?;
        Ok(())
    } else {
        simple_vector_set(ctx, sequence, index, value)
    }
}
fn call_predicate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let mut designator = function;
    let designator_root = push_root(ctx, &mut designator);
    let mut rooted = args.to_vec();
    let roots = rooted
        .iter_mut()
        .map(|word| push_root(ctx, word))
        .collect::<Vec<_>>();
    let result = (|| {
        let designator = FunctionDesignator::try_from_word(ctx, designator)?;
        let mut caller = BuiltinFunctionCaller;
        let mut values = MultipleValues::new();
        caller.call_function(
            ctx,
            runtime,
            designator,
            FunctionArguments::new(&rooted),
            &mut values,
        )
    })();
    let valid =
        roots.into_iter().rev().all(|token| pop_root(ctx, token)) && pop_root(ctx, designator_root);
    if valid {
        result
    } else {
        Err(ObjectError::Layout)
    }
}
fn key(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    key: Word,
    value: Word,
) -> Result<Word, ObjectError> {
    if key == Word::NIL {
        Ok(value)
    } else {
        call_predicate(ctx, runtime, key, &[value])
    }
}
pub(super) fn matches(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    a: Word,
    b: Word,
    opts: Options,
) -> Result<bool, ObjectError> {
    let rooted = [a, b, opts.key, opts.test, opts.test_not];
    ncl_object::with_roots(ctx, &rooted, |ctx, rooted| {
        let mut keyed_a = key(
            ctx,
            runtime,
            **rooted.get(2).ok_or(ObjectError::Layout)?,
            **rooted.first().ok_or(ObjectError::Layout)?,
        )?;
        ncl_object::with_root(ctx, &mut keyed_a, |ctx, keyed_a| {
            let mut keyed_b = key(
                ctx,
                runtime,
                **rooted.get(2).ok_or(ObjectError::Layout)?,
                **rooted.get(1).ok_or(ObjectError::Layout)?,
            )?;
            ncl_object::with_root(ctx, &mut keyed_b, |ctx, keyed_b| {
                if **rooted.get(3).ok_or(ObjectError::Layout)? != Word::NIL {
                    Ok(call_predicate(
                        ctx,
                        runtime,
                        **rooted.get(3).ok_or(ObjectError::Layout)?,
                        &[*keyed_a, *keyed_b],
                    )? != Word::NIL)
                } else if **rooted.get(4).ok_or(ObjectError::Layout)? != Word::NIL {
                    Ok(call_predicate(
                        ctx,
                        runtime,
                        **rooted.get(4).ok_or(ObjectError::Layout)?,
                        &[*keyed_a, *keyed_b],
                    )? == Word::NIL)
                } else {
                    Ok(*keyed_a == *keyed_b)
                }
            })
        })
    })
}
fn compare(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    a: Word,
    b: Word,
) -> Result<bool, ObjectError> {
    Ok(call_predicate(ctx, runtime, function, &[a, b])? != Word::NIL)
}
pub fn sort(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    sequence: Word,
    predicate: Word,
    key_fn: Word,
    stable: bool,
) -> Result<Word, ObjectError> {
    let mut values = sequence_values(ctx, sequence)?;
    let function_roots = [sequence, predicate, key_fn];
    ncl_object::with_roots(ctx, &function_roots, |ctx, function_roots| {
        let roots = values
            .iter_mut()
            .map(|word| push_root(ctx, word))
            .collect::<Vec<_>>();
        let result = {
            let key_values = vec![Word::NIL; values.len()];
            ncl_object::with_rooted_slice(ctx, &key_values, |ctx, key_values| {
                for (key_value, value) in key_values.iter_mut().zip(values.iter()) {
                    *key_value = if **function_roots.get(2).ok_or(ObjectError::Layout)? == Word::NIL
                    {
                        *value
                    } else {
                        key(
                            ctx,
                            runtime,
                            **function_roots.get(2).ok_or(ObjectError::Layout)?,
                            *value,
                        )?
                    };
                }
                let mut order: Vec<usize> = Vec::with_capacity(values.len());
                for index in 0..values.len() {
                    let mut position = order.len();
                    for (offset, &other) in order.iter().enumerate() {
                        if compare(
                            ctx,
                            runtime,
                            **function_roots.get(1).ok_or(ObjectError::Layout)?,
                            *key_values.get(index).ok_or(ObjectError::Layout)?,
                            *key_values.get(other).ok_or(ObjectError::Layout)?,
                        )? {
                            position = offset;
                            break;
                        }
                    }
                    if !stable
                        && position < order.len()
                        && *key_values.get(index).ok_or(ObjectError::Layout)?
                            == *key_values
                                .get(*order.get(position).ok_or(ObjectError::Layout)?)
                                .ok_or(ObjectError::Layout)?
                    {
                        position += 1;
                    }
                    order.insert(position, index);
                }
                for (i, index) in order.into_iter().enumerate() {
                    let value = *values.get(index).ok_or(ObjectError::Layout)?;
                    set_sequence_value(
                        ctx,
                        **function_roots.first().ok_or(ObjectError::Layout)?,
                        i,
                        value,
                    )?;
                }
                Ok(**function_roots.first().ok_or(ObjectError::Layout)?)
            })
        };
        let valid = roots.into_iter().rev().all(|token| pop_root(ctx, token));
        if valid {
            result
        } else {
            Err(ObjectError::Layout)
        }
    })
}
#[allow(dead_code)]
pub fn stable_sort(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    sequence: Word,
    predicate: Word,
    key_fn: Word,
) -> Result<Word, ObjectError> {
    sort(ctx, runtime, sequence, predicate, key_fn, true)
}
pub(super) fn set_operation(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    first: Word,
    second: Word,
    opts: Options,
    exclusive: bool,
    difference: bool,
) -> Result<Word, ObjectError> {
    let mut rows = vec![sequence_values(ctx, first)?, sequence_values(ctx, second)?];
    rooted_nested(ctx, &mut rows, |ctx, rows| {
        with_options(ctx, opts, |ctx, opts| {
            let candidates = if difference {
                (0..rows.first().map_or(0, Vec::len))
                    .map(|index| (0, index))
                    .collect::<Vec<_>>()
            } else {
                rows.iter()
                    .enumerate()
                    .flat_map(|(row, values)| (0..values.len()).map(move |index| (row, index)))
                    .collect()
            };
            let result = vec![Word::NIL; candidates.len()];
            ncl_object::with_rooted_slice(ctx, &result, |ctx, result| {
                let mut result_len = 0;
                for &(candidate_row, candidate_index) in &candidates {
                    let mut in_left = false;
                    for candidate in rows.first().ok_or(ObjectError::Layout)? {
                        let value = *rows
                            .get(candidate_row)
                            .and_then(|row| row.get(candidate_index))
                            .ok_or(ObjectError::Layout)?;
                        if matches(ctx, runtime, value, *candidate, opts)? {
                            in_left = true;
                            break;
                        }
                    }
                    let mut in_right = false;
                    for candidate in rows.get(1).ok_or(ObjectError::Layout)? {
                        let value = *rows
                            .get(candidate_row)
                            .and_then(|row| row.get(candidate_index))
                            .ok_or(ObjectError::Layout)?;
                        if matches(ctx, runtime, value, *candidate, opts)? {
                            in_right = true;
                            break;
                        }
                    }
                    let include = if difference {
                        in_left && !in_right
                    } else if exclusive {
                        in_left != in_right
                    } else {
                        true
                    };
                    let mut duplicate = false;
                    for candidate in result.iter().take(result_len) {
                        let value = *rows
                            .get(candidate_row)
                            .and_then(|row| row.get(candidate_index))
                            .ok_or(ObjectError::Layout)?;
                        if matches(ctx, runtime, value, *candidate, opts)? {
                            duplicate = true;
                            break;
                        }
                    }
                    if include && !duplicate {
                        let value = *rows
                            .get(candidate_row)
                            .and_then(|row| row.get(candidate_index))
                            .ok_or(ObjectError::Layout)?;
                        *result.get_mut(result_len).ok_or(ObjectError::Layout)? = value;
                        result_len += 1;
                    }
                }
                list_from(
                    ctx,
                    runtime,
                    result.get(..result_len).ok_or(ObjectError::Layout)?,
                )
            })
        })
    })
}
pub fn union(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    first: Word,
    second: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    set_operation(ctx, runtime, first, second, opts, false, false)
}
