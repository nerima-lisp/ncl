#[path = "order_sets_assoc.rs"]
mod assoc;
pub use assoc::{assoc, member, rassoc};
#[path = "order_sets_extra.rs"]
mod extra;
pub use extra::{adjoin, intersection, set_difference, set_exclusive_or, subsetp};
use ncl_object::{
    Handle, HandleVec, Local, ObjectError, ObjectRef, Runtime, Scope, ThreadContext, Word, car,
    cdr, classify_object, rplaca, simple_vector_length, simple_vector_ref, simple_vector_set,
};
use std::ops::Range;

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
    f: impl FnOnce(&mut ThreadContext, &mut [Word], &[Range<usize>]) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let values = words.iter().flatten().copied().collect::<Vec<_>>();
    let mut start = 0;
    let ranges = words
        .iter()
        .map(|row| {
            let range = start..start + row.len();
            start = range.end;
            range
        })
        .collect::<Vec<_>>();
    ncl_object::with_rooted_slice(ctx, &values, |ctx, roots| f(ctx, roots, &ranges))
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
    f: impl FnOnce(&mut ThreadContext, &mut [Word]) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let root_values = [opts.key, opts.test, opts.test_not];
    ncl_object::with_rooted_slice(ctx, &root_values, f)
}
pub(super) fn options_from_roots(roots: &[Word]) -> Result<Options, ObjectError> {
    Ok(Options {
        key: *roots.first().ok_or(ObjectError::Layout)?,
        test: *roots.get(1).ok_or(ObjectError::Layout)?,
        test_not: *roots.get(2).ok_or(ObjectError::Layout)?,
    })
}
pub(super) fn row_len(ranges: &[Range<usize>], row: usize) -> Result<usize, ObjectError> {
    Ok(ranges.get(row).ok_or(ObjectError::Layout)?.len())
}
fn candidate_positions(
    ranges: &[Range<usize>],
    difference: bool,
) -> Result<Vec<(usize, usize)>, ObjectError> {
    if difference {
        Ok((0..row_len(ranges, 0)?).map(|index| (0, index)).collect())
    } else {
        Ok(ranges
            .iter()
            .enumerate()
            .flat_map(|(row, range)| (0..range.len()).map(move |index| (row, index)))
            .collect())
    }
}
pub(super) fn row_value(
    roots: &[Word],
    ranges: &[Range<usize>],
    row: usize,
    index: usize,
) -> Result<Word, ObjectError> {
    let range = ranges.get(row).ok_or(ObjectError::Layout)?;
    let offset = range
        .start
        .checked_add(index)
        .filter(|offset| *offset < range.end)
        .ok_or(ObjectError::Layout)?;
    roots.get(offset).copied().ok_or(ObjectError::Layout)
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
        let argument = *args.get(i).ok_or(ObjectError::Layout)?;
        let name = if argument != Word::NIL
            && matches!(classify_object(ctx, argument), ObjectRef::Symbol(_))
        {
            Some(symbol_name(ctx, argument)?)
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
            positional.push(argument);
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
#[path = "order_sets_callbacks.rs"]
mod callbacks;
pub(super) use callbacks::matches;
use callbacks::{compare, key};

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
        let predicate_handle = *handles.iter().nth(1).ok_or(ObjectError::Layout)?;
        let key_handle = *handles.iter().nth(2).ok_or(ObjectError::Layout)?;
        let value_handles = handles.iter().skip(3).copied().collect::<Vec<_>>();
        let key_values = scope.root_many(&vec![Local::from_word(Word::NIL); values.len()]);
        for (key_value_handle, value_handle) in key_values.iter().zip(value_handles.iter()) {
            let value = word(scope, *value_handle);
            let key_word = word(scope, key_handle);
            let keyed = if key_word == Word::NIL {
                value
            } else {
                key(scope.context_mut(), runtime, key_word, value)?
            };
            scope.set(*key_value_handle, Local::from_word(keyed));
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
                let predicate_word = word(scope, predicate_handle);
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
    scoped_rows(ctx, &rows, |ctx, row_roots, row_ranges| {
        with_options(ctx, opts, |ctx, option_roots| {
            let candidates = candidate_positions(row_ranges, difference)?;
            let result = vec![Word::NIL; candidates.len()];
            let mut result_rows = vec![0; candidates.len()];
            ncl_object::with_rooted_slice(ctx, &result, |ctx, rooted_result| {
                let mut result_len = 0;
                for &(candidate_row, candidate_index) in &candidates {
                    let mut in_left = candidate_row == 0;
                    if candidate_row == 1 {
                        for left_index in 0..row_len(row_ranges, 0)? {
                            let candidate = row_value(row_roots, row_ranges, 0, left_index)?;
                            let value =
                                row_value(row_roots, row_ranges, candidate_row, candidate_index)?;
                            if matches(
                                ctx,
                                runtime,
                                candidate,
                                value,
                                options_from_roots(option_roots)?,
                            )? {
                                in_left = true;
                                break;
                            }
                        }
                    }
                    let mut in_right = candidate_row == 1;
                    if candidate_row == 0 {
                        for right_index in 0..row_len(row_ranges, 1)? {
                            let value =
                                row_value(row_roots, row_ranges, candidate_row, candidate_index)?;
                            let candidate = row_value(row_roots, row_ranges, 1, right_index)?;
                            if matches(
                                ctx,
                                runtime,
                                value,
                                candidate,
                                options_from_roots(option_roots)?,
                            )? {
                                in_right = true;
                                break;
                            }
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
                    for index in 0..result_len {
                        let value =
                            row_value(row_roots, row_ranges, candidate_row, candidate_index)?;
                        let candidate = *rooted_result.get(index).ok_or(ObjectError::Layout)?;
                        let row = *result_rows.get(index).ok_or(ObjectError::Layout)?;
                        let (first, second) = if row == 0 && candidate_row == 1 {
                            (candidate, value)
                        } else {
                            (value, candidate)
                        };
                        if matches(
                            ctx,
                            runtime,
                            first,
                            second,
                            options_from_roots(option_roots)?,
                        )? {
                            duplicate = true;
                            break;
                        }
                    }
                    if include && !duplicate {
                        let value =
                            row_value(row_roots, row_ranges, candidate_row, candidate_index)?;
                        *rooted_result
                            .get_mut(result_len)
                            .ok_or(ObjectError::Layout)? = value;
                        *result_rows.get_mut(result_len).ok_or(ObjectError::Layout)? =
                            candidate_row;
                        result_len += 1;
                    }
                }
                list_from(
                    ctx,
                    runtime,
                    rooted_result.get(..result_len).ok_or(ObjectError::Layout)?,
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
