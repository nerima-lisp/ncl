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
    BuiltinFunctionCaller, FunctionArguments, FunctionCaller, Handle, HandleVec, Local,
    MultipleValues, ObjectError, ObjectRef, Runtime, Scope, ThreadContext, Word, car, cdr,
    classify_object, rplaca, simple_vector_length, simple_vector_ref, simple_vector_set,
};

pub(super) fn with_scope<'ctx, T>(
    ctx: &'ctx mut ThreadContext,
    values: &[Word],
    f: impl FnOnce(&mut Scope<'ctx>, &HandleVec<'ctx>) -> T,
) -> T {
    let mut scope = Scope::new(ctx);
    let locals = values
        .iter()
        .copied()
        .map(Local::from_word)
        .collect::<Vec<_>>();
    let handles = scope.root_many(&locals);
    f(&mut scope, &handles)
}

fn word<'scope>(scope: &Scope<'scope>, handle: Handle<'scope>) -> Word {
    scope.get(handle).as_word()
}

pub(super) fn list_from(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    with_scope(ctx, values, |scope, handles| {
        let result = scope.root(Local::from_word(Word::NIL));
        for handle in handles.iter().rev() {
            let first = word(scope, *handle);
            let second = word(scope, result);
            let next = ncl_object::make_cons(scope.context_mut(), runtime, first, second)?;
            scope.set(result, Local::from_word(next));
        }
        Ok(word(scope, result))
    })
}
pub(super) fn scoped_rows<T>(
    ctx: &mut ThreadContext,
    words: &[Vec<Word>], // check-added-lines: allow(index) slice type
    f: impl FnOnce(&mut ThreadContext, &[Vec<Word>]) -> T,
) -> T {
    let values = words.iter().flatten().copied().collect::<Vec<_>>();
    with_scope(ctx, &values, |scope, _| f(scope.context_mut(), words))
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
    let root_values = [opts.key, opts.test, opts.test_not];
    with_scope(ctx, &root_values, |scope, handles| {
        let options = Options {
            key: word(scope, *handles.iter().next().ok_or(ObjectError::Layout)?),
            test: word(scope, *handles.iter().nth(1).ok_or(ObjectError::Layout)?),
            test_not: word(scope, *handles.iter().nth(2).ok_or(ObjectError::Layout)?),
        };
        f(scope.context_mut(), options)
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
    ctx: &ThreadContext,
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
    let mut values = Vec::with_capacity(args.len() + 1);
    values.push(function);
    values.extend_from_slice(args);
    with_scope(ctx, &values, |scope, handles| {
        let designator = FunctionDesignator::try_from_word(
            scope.context(),
            word(scope, *handles.iter().next().ok_or(ObjectError::Layout)?),
        )?;
        let mut caller = BuiltinFunctionCaller;
        let mut values = MultipleValues::new();
        let args = handles
            .iter()
            .skip(1)
            .map(|handle| word(scope, *handle))
            .collect::<Vec<_>>();
        caller.call_function(
            scope.context_mut(),
            runtime,
            designator,
            FunctionArguments::new(&args),
            &mut values,
        )
    })
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
    let root_values = [a, b, opts.key, opts.test, opts.test_not];
    with_scope(ctx, &root_values, |scope, handles| {
        let key_fn = word(scope, *handles.iter().nth(2).ok_or(ObjectError::Layout)?);
        let first = word(scope, *handles.iter().next().ok_or(ObjectError::Layout)?);
        let second = word(scope, *handles.iter().nth(1).ok_or(ObjectError::Layout)?);
        let keyed_a_word = key(scope.context_mut(), runtime, key_fn, first)?;
        let keyed_a = scope.root(Local::from_word(keyed_a_word));
        let keyed_second_word = key(scope.context_mut(), runtime, key_fn, second)?;
        let keyed_b = scope.root(Local::from_word(keyed_second_word));
        let left = word(scope, keyed_a);
        let right = word(scope, keyed_b);
        let test = word(scope, *handles.iter().nth(3).ok_or(ObjectError::Layout)?);
        let test_not = word(scope, *handles.iter().nth(4).ok_or(ObjectError::Layout)?);
        if test != Word::NIL {
            Ok(call_predicate(scope.context_mut(), runtime, test, &[left, right])? != Word::NIL)
        } else if test_not != Word::NIL {
            Ok(
                call_predicate(scope.context_mut(), runtime, test_not, &[left, right])?
                    == Word::NIL,
            )
        } else {
            Ok(left == right)
        }
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
    let values = sequence_values(ctx, sequence)?;
    let mut roots = vec![sequence, predicate, key_fn];
    roots.extend_from_slice(&values);
    with_scope(ctx, &roots, |scope, handles| {
        let sequence_handle = *handles.iter().next().ok_or(ObjectError::Layout)?;
        let predicate_word = word(scope, *handles.iter().nth(1).ok_or(ObjectError::Layout)?);
        let key_word = word(scope, *handles.iter().nth(2).ok_or(ObjectError::Layout)?);
        let value_handles = handles.iter().skip(3).copied().collect::<Vec<_>>();
        let key_values = scope.root_many(&vec![Local::from_word(Word::NIL); values.len()]);
        for (key_handle, value_handle) in key_values.iter().zip(value_handles.iter()) {
            let value = word(scope, *value_handle);
            let keyed = if key_word == Word::NIL {
                value
            } else {
                key(scope.context_mut(), runtime, key_word, value)?
            };
            scope.set(*key_handle, Local::from_word(keyed));
        }
        let mut order: Vec<usize> = Vec::with_capacity(values.len());
        for index in 0..values.len() {
            let mut position = order.len();
            for (offset, &other) in order.iter().enumerate() {
                let current_key = word(
                    scope,
                    *key_values.iter().nth(index).ok_or(ObjectError::Layout)?,
                );
                let other_key = word(
                    scope,
                    *key_values.iter().nth(other).ok_or(ObjectError::Layout)?,
                );
                if compare(
                    scope.context_mut(),
                    runtime,
                    predicate_word,
                    current_key,
                    other_key,
                )? {
                    position = offset;
                    break;
                }
            }
            if !stable
                && position < order.len()
                && word(
                    scope,
                    *key_values.iter().nth(index).ok_or(ObjectError::Layout)?,
                ) == word(
                    scope,
                    *key_values
                        .iter()
                        .nth(*order.get(position).ok_or(ObjectError::Layout)?)
                        .ok_or(ObjectError::Layout)?,
                )
            {
                position += 1;
            }
            order.insert(position, index);
        }
        for (i, index) in order.into_iter().enumerate() {
            let value = word(scope, *value_handles.get(index).ok_or(ObjectError::Layout)?);
            let sequence_word = word(scope, sequence_handle);
            set_sequence_value(scope.context_mut(), sequence_word, i, value)?;
        }
        Ok(word(scope, sequence_handle))
    })
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
    let rows = vec![sequence_values(ctx, first)?, sequence_values(ctx, second)?];
    scoped_rows(ctx, &rows, |ctx, rows| {
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
            let mut result = vec![Word::NIL; candidates.len()];
            {
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
            }
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
