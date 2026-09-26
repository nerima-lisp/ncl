//! Higher-order sequence operations.

#![allow(dead_code)]

use ncl_object::{
    BuiltinFunctionCaller, FunctionArguments, FunctionCaller, FunctionDesignator, List,
    MultipleValues, ObjectError, Runtime, Sequence, ThreadContext, Word, make_simple_vector,
    pop_root, push_root, simple_vector_length, simple_vector_ref,
};

fn list_word(list: List) -> Word {
    match list {
        List::Nil => Word::NIL,
        List::Cons(cons) => cons.into(),
    }
}

fn values(ctx: &mut ThreadContext, sequence: Sequence) -> Result<Vec<Word>, ObjectError> {
    match sequence {
        Sequence::List(list) => {
            let mut result = Vec::new();
            let mut cursor = list_word(list);
            while cursor != Word::NIL {
                if !cursor.is_cons() {
                    return Err(ObjectError::TypeError);
                }
                result.push(ncl_object::car(ctx, cursor)?);
                cursor = ncl_object::cdr(ctx, cursor)?;
            }
            Ok(result)
        }
        Sequence::Vector(vector) => {
            let word: Word = vector.into();
            (0..simple_vector_length(ctx, word)?)
                .map(|index| simple_vector_ref(ctx, word, index))
                .collect()
        }
        Sequence::String(_) => Err(ObjectError::TypeError),
    }
}

fn rooted<T>(
    ctx: &mut ThreadContext,
    words: &mut [Word],
    f: impl FnOnce(&mut ThreadContext, &[Word]) -> T,
) -> T {
    let tokens = words
        .iter_mut()
        .map(|word| push_root(ctx, word))
        .collect::<Vec<_>>();
    let result = f(ctx, words);
    tokens.into_iter().rev().all(|token| pop_root(ctx, token));
    result
}

fn rooted_nested<T>(
    ctx: &mut ThreadContext,
    words: &mut [Vec<Word>],
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

fn sequence_result(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    result_type: Sequence,
    values: &mut [Word],
) -> Result<Word, ObjectError> {
    let tokens = values
        .iter_mut()
        .map(|value| push_root(ctx, value))
        .collect::<Vec<_>>();
    let result = match result_type {
        Sequence::List(_) => super::list_from(ctx, runtime, values),
        Sequence::Vector(_) => make_simple_vector(ctx, runtime, values),
        Sequence::String(_) => Err(ObjectError::TypeError),
    };
    if tokens.into_iter().rev().all(|token| pop_root(ctx, token)) {
        result
    } else {
        Err(ObjectError::Layout)
    }
}

fn call<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    callback: FunctionDesignator,
    args: &[Word],
) -> Result<(Word, MultipleValues), ObjectError> {
    let mut values = MultipleValues::new();
    let result = caller.call_function(
        ctx,
        runtime,
        callback,
        FunctionArguments::new(args),
        &mut values,
    )?;
    Ok((result, values))
}

fn callback(ctx: &ThreadContext, word: Word) -> Result<FunctionDesignator, ObjectError> {
    FunctionDesignator::try_from_word(ctx, word)
}

/// MAP over zero or more list/vector sequences.
pub fn map<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    result_type: Sequence,
    function: Word,
    sequences: &[Sequence],
    caller: &mut C,
) -> Result<Word, ObjectError> {
    let callback_word = function;
    let mut callback_roots = [callback_word];
    rooted(ctx, &mut callback_roots, |ctx, rooted_callback| {
        let callback = callback(ctx, rooted_callback[0])?;
        let mut sources = sequences
            .iter()
            .map(|sequence| values(ctx, *sequence))
            .collect::<Result<Vec<_>, _>>()?;
        let length = sources.iter().map(Vec::len).min().unwrap_or(0);
        rooted_nested(ctx, &mut sources, |ctx, sources| {
            let mut result = Vec::with_capacity(length);
            let mut result_roots = Vec::new();
            for index in 0..length {
                let args = sources
                    .iter()
                    .map(|source| source[index])
                    .collect::<Vec<_>>();
                let mut value = call(ctx, runtime, caller, callback, &args)?.0;
                result_roots.push(push_root(ctx, &mut value));
                result.push(value);
            }
            let output = sequence_result(ctx, runtime, result_type, &mut result);
            if result_roots
                .into_iter()
                .rev()
                .all(|token| pop_root(ctx, token))
            {
                output
            } else {
                Err(ObjectError::Layout)
            }
        })
    })
}

/// MAP-INTO the destination list or vector, returning the destination.
pub fn map_into<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    destination: Word,
    function: Word,
    sequences: &[Sequence],
    caller: &mut C,
) -> Result<Word, ObjectError> {
    let mut roots = [destination, function];
    rooted(ctx, &mut roots, |ctx, roots| {
        let destination_sequence = super::sequence_value(ctx, roots[0])?;
        let mut destination_values = values(ctx, destination_sequence)?;
        let mut sources = sequences
            .iter()
            .map(|sequence| values(ctx, *sequence))
            .collect::<Result<Vec<_>, _>>()?;
        let length = std::iter::once(destination_values.len())
            .chain(sources.iter().map(Vec::len))
            .min()
            .unwrap_or(0);
        let callback = callback(ctx, roots[1])?;
        rooted_nested(ctx, &mut sources, |ctx, sources| {
            for index in 0..length {
                let args = sources
                    .iter()
                    .map(|source| source[index])
                    .collect::<Vec<_>>();
                destination_values[index] = call(ctx, runtime, caller, callback, &args)?.0;
            }
            match destination_sequence {
                Sequence::List(list) => {
                    let mut cursor = list_word(list);
                    for value in destination_values.into_iter().take(length) {
                        ncl_object::rplaca(ctx, cursor, value)?;
                        cursor = ncl_object::cdr(ctx, cursor)?;
                    }
                }
                Sequence::Vector(vector) => {
                    let word: Word = vector.into();
                    for (index, value) in destination_values.into_iter().take(length).enumerate() {
                        ncl_object::simple_vector_set(ctx, word, index, value)?;
                    }
                }
                Sequence::String(_) => return Err(ObjectError::TypeError),
            }
            Ok(roots[0])
        })
    })
}

/// REDUCE with optional initial value and direction.
pub fn reduce<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    sequence: Sequence,
    initial: Option<Word>,
    from_end: bool,
    caller: &mut C,
) -> Result<Word, ObjectError> {
    let mut roots = [function, initial.unwrap_or(Word::NIL)];
    rooted(ctx, &mut roots, |ctx, roots| {
        let mut items = values(ctx, sequence)?;
        if from_end {
            items.reverse();
        }
        let callback = callback(ctx, roots[0])?;
        let mut index = 0;
        let mut accumulator = if initial.is_some() {
            roots[1]
        } else {
            *items.first().ok_or(ObjectError::TypeError)?
        };
        if initial.is_none() {
            index = 1;
        }
        rooted(ctx, &mut items, |ctx, items| {
            let accumulator_token = push_root(ctx, &mut accumulator);
            while index < items.len() {
                let args = [accumulator, items[index]];
                accumulator = call(ctx, runtime, caller, callback, &args)?.0;
                index += 1;
            }
            let valid = pop_root(ctx, accumulator_token);
            if valid {
                Ok(accumulator)
            } else {
                Err(ObjectError::Layout)
            }
        })
    })
}

/// EVERY, SOME, NOTANY, and NOTEVERY. The operation is selected by `kind`.
pub fn predicate<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    kind: Predicate,
    function: Word,
    sequences: &[Sequence],
    caller: &mut C,
) -> Result<Word, ObjectError> {
    let mut callback_root = [function];
    rooted(ctx, &mut callback_root, |ctx, root| {
        let callback = callback(ctx, root[0])?;
        let mut source_values = sequences
            .iter()
            .map(|sequence| values(ctx, *sequence))
            .collect::<Result<Vec<_>, _>>()?;
        rooted_nested(ctx, &mut source_values, |ctx, source_values| {
            let length = source_values.iter().map(Vec::len).min().unwrap_or(0);
            let mut result = matches!(kind, Predicate::Every | Predicate::NotAny);
            for index in 0..length {
                let args = source_values
                    .iter()
                    .map(|source| source[index])
                    .collect::<Vec<_>>();
                let truth = call(ctx, runtime, caller, callback, &args)?.0 != Word::NIL;
                result = match kind {
                    Predicate::Every | Predicate::Some => truth,
                    Predicate::NotAny | Predicate::NotEvery => !truth,
                };
                if matches!(kind, Predicate::Every | Predicate::NotEvery) && !truth
                    || matches!(kind, Predicate::Some | Predicate::NotAny) && truth
                {
                    break;
                }
            }
            Ok(if result { Word::TRUE } else { Word::NIL })
        })
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Predicate {
    Every,
    Some,
    NotAny,
    NotEvery,
}

fn list_map<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    lists: &[Word],
    caller: &mut C,
    tails: bool,
) -> Result<Vec<Word>, ObjectError> {
    let sequences = lists
        .iter()
        .map(|&list| super::sequence_value(ctx, list))
        .collect::<Result<Vec<_>, _>>()?;
    let mut sources = sequences
        .iter()
        .map(|sequence| values(ctx, *sequence))
        .collect::<Result<Vec<_>, _>>()?;
    let length = sources.iter().map(Vec::len).min().unwrap_or(0);
    let mut list_roots = lists.to_vec();
    rooted(ctx, &mut list_roots, |ctx, lists| {
        let mut function_root = [function];
        rooted(ctx, &mut function_root, |ctx, function_root| {
            rooted_nested(ctx, &mut sources, |ctx, sources| {
                let mut result = Vec::with_capacity(length);
                let mut result_roots = Vec::new();
                let callback = callback(ctx, function_root[0])?;
                for index in 0..length {
                    let args = if tails {
                        lists
                            .iter()
                            .map(|&list| nth_tail(ctx, list, index))
                            .collect::<Result<Vec<_>, _>>()?
                    } else {
                        sources.iter().map(|source| source[index]).collect()
                    };
                    let mut value = call(ctx, runtime, caller, callback, &args)?.0;
                    result_roots.push(push_root(ctx, &mut value));
                    result.push(value);
                }
                let valid = result_roots
                    .into_iter()
                    .rev()
                    .all(|token| pop_root(ctx, token));
                if valid {
                    Ok(result)
                } else {
                    Err(ObjectError::Layout)
                }
            })
        })
    })
}

fn nth_tail(ctx: &mut ThreadContext, mut list: Word, index: usize) -> Result<Word, ObjectError> {
    for _ in 0..index {
        list = ncl_object::cdr(ctx, list)?;
    }
    Ok(list)
}

pub fn mapcar<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    lists: &[Word],
    caller: &mut C,
) -> Result<Word, ObjectError> {
    let result = list_map(ctx, runtime, function, lists, caller, false)?;
    super::list_from(ctx, runtime, &result)
}

pub fn mapc<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    lists: &[Word],
    caller: &mut C,
) -> Result<Word, ObjectError> {
    list_map(ctx, runtime, function, lists, caller, false)?;
    lists.first().copied().ok_or(ObjectError::TypeError)
}

pub fn maplist<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    lists: &[Word],
    caller: &mut C,
) -> Result<Word, ObjectError> {
    let result = list_map(ctx, runtime, function, lists, caller, true)?;
    super::list_from(ctx, runtime, &result)
}

pub fn mapl<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    lists: &[Word],
    caller: &mut C,
) -> Result<Word, ObjectError> {
    list_map(ctx, runtime, function, lists, caller, true)?;
    lists.first().copied().ok_or(ObjectError::TypeError)
}

pub fn mapcan<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    lists: &[Word],
    caller: &mut C,
) -> Result<Word, ObjectError> {
    let results = list_map(ctx, runtime, function, lists, caller, false)?;
    let mut flattened = Vec::new();
    for result in results {
        flattened.extend(values(ctx, super::sequence_value(ctx, result)?)?);
    }
    super::list_from(ctx, runtime, &flattened)
}

pub fn mapcon<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    lists: &[Word],
    caller: &mut C,
) -> Result<Word, ObjectError> {
    let results = list_map(ctx, runtime, function, lists, caller, true)?;
    let mut flattened = Vec::new();
    for result in results {
        flattened.extend(values(ctx, super::sequence_value(ctx, result)?)?);
    }
    super::list_from(ctx, runtime, &flattened)
}

/// Convenience entry point for callers that do not need a custom function caller.
pub const fn builtin_caller() -> BuiltinFunctionCaller {
    BuiltinFunctionCaller
}
