//! Common Lisp selection and matching operations.

#![allow(dead_code)]
//!
//! The implementation keeps a rooted snapshot of a sequence while calling
//! Lisp functions.  This is important: a key or predicate may allocate and
//! move every object in the sequence.

use ncl_object::{
    FunctionArguments, FunctionCaller, FunctionDesignator, List, MultipleValues, ObjectError,
    ObjectRef, Runtime, Sequence, ThreadContext, Word, car, cdr, classify_object,
    simple_vector_length, simple_vector_ref, string_length, string_ref,
};
#[path = "selection_options.rs"]
mod selection_options;
#[path = "selection_values.rs"]
mod selection_values;
pub use self::selection_options::{PairSelectionOptions, parse_pair_options};
pub use self::selection_options::{
    SelectionOptionMode, SelectionOptions, parse_options, parse_options_single,
    parse_options_with_minimum,
};

pub fn list_from_values(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &mut [Word], // check-added-lines: allow(index) slice type
) -> Result<Word, ObjectError> {
    selection_values::list_from_values(ctx, runtime, values)
}

pub fn object_sequence(ctx: &ThreadContext, word: Word) -> Result<Sequence, ObjectError> {
    selection_values::object_sequence(ctx, word)
}

fn scope_roots<T>(
    ctx: &mut ThreadContext,
    values: &[Word],
    f: impl FnOnce(&mut ThreadContext, &mut [Word]) -> Result<T, ObjectError>, // check-added-lines: allow(index) slice type
) -> Result<T, ObjectError> {
    ncl_object::with_rooted_slice(ctx, values, f)
}

fn scope_rooted_slice<T>(
    ctx: &mut ThreadContext,
    values: &[Word],
    f: impl FnOnce(&mut ThreadContext, &mut [Word]) -> Result<T, ObjectError>, // check-added-lines: allow(index) slice type
) -> Result<T, ObjectError> {
    ncl_object::with_rooted_slice(ctx, values, f)
}

pub fn sequence_values(ctx: &ThreadContext, sequence: Sequence) -> Result<Vec<Word>, ObjectError> {
    let mut values = Vec::new();
    match sequence {
        Sequence::List(list) => {
            let mut cursor = match list {
                List::Nil => Word::NIL,
                List::Cons(value) => value.into(),
            };
            while cursor != Word::NIL {
                if !cursor.is_cons() {
                    return Err(ObjectError::TypeError);
                }
                values.push(car(ctx, cursor)?);
                cursor = cdr(ctx, cursor)?;
            }
        }
        Sequence::Vector(vector) => {
            let word: Word = vector.into();
            for index in 0..simple_vector_length(ctx, word)? {
                values.push(simple_vector_ref(ctx, word, index)?);
            }
        }
        Sequence::String(string) => {
            let word: Word = string.into();
            for index in 0..string_length(ctx, word)? {
                values.push(Word::character(u32::from(string_ref(ctx, word, index)?)));
            }
        }
    }
    Ok(values)
}

fn function(ctx: &ThreadContext, word: Word) -> Result<FunctionDesignator, ObjectError> {
    FunctionDesignator::try_from_word(ctx, word)
}

fn call_one<C: FunctionCaller>(
    caller: &mut C,
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    designator: Word,
    argument: Word,
) -> Result<Word, ObjectError> {
    let roots = [designator, argument];
    scope_roots(ctx, &roots, |ctx, roots| {
        let mut values = MultipleValues::new();
        caller.call_function(
            ctx,
            runtime,
            function(ctx, *roots.first().ok_or(ObjectError::Layout)?)?,
            FunctionArguments::new(&[*roots.get(1).ok_or(ObjectError::Layout)?]),
            &mut values,
        )
    })
}

fn matches<C: FunctionCaller>(
    caller: &mut C,
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    options: SelectionOptions,
    item: Word,
    object: Word,
) -> Result<bool, ObjectError> {
    let keyed = if let Some(key) = options.key {
        call_one(caller, ctx, runtime, key, item)?
    } else {
        item
    };
    let predicate_roots = [
        keyed,
        object,
        options.key.unwrap_or(Word::NIL),
        options.test.unwrap_or(Word::NIL),
        options.test_not.unwrap_or(Word::NIL),
    ];
    scope_roots(ctx, &predicate_roots, |ctx, roots| {
        let keyed = *roots.first().ok_or(ObjectError::Layout)?;
        let object = *roots.get(1).ok_or(ObjectError::Layout)?;
        let result = if options.test.is_some() {
            let designator = function(ctx, *roots.get(3).ok_or(ObjectError::Layout)?)?;
            let mut values = MultipleValues::new();
            let args = [object, keyed];
            caller.call_function(
                ctx,
                runtime,
                designator,
                FunctionArguments::new(&args),
                &mut values,
            )?
        } else if options.test_not.is_some() {
            let designator = function(ctx, *roots.get(4).ok_or(ObjectError::Layout)?)?;
            let mut values = MultipleValues::new();
            let args = [object, keyed];
            caller.call_function(
                ctx,
                runtime,
                designator,
                FunctionArguments::new(&args),
                &mut values,
            )?
        } else {
            Word::NIL
        };
        let keyed = *roots.first().ok_or(ObjectError::Layout)?;
        let object = *roots.get(1).ok_or(ObjectError::Layout)?;
        let truth = result != Word::NIL;
        Ok(if options.test_not.is_some() {
            !truth
        } else if options.test.is_some() {
            truth
        } else {
            keyed == object
        })
    })
}

fn bounds(options: SelectionOptions, length: usize) -> Result<(usize, usize), ObjectError> {
    bounds_from(options.start, options.end, length)
}

fn bounds_from(
    start: usize,
    end: Option<usize>,
    length: usize,
) -> Result<(usize, usize), ObjectError> {
    let end = end.unwrap_or(length);
    if start > end || end > length {
        Err(ObjectError::TypeError)
    } else {
        Ok((start, end))
    }
}

fn index_word(index: usize) -> Result<Word, ObjectError> {
    i64::try_from(index)
        .map(Word::fixnum)
        .map_err(|_| ObjectError::TypeError)
}

#[allow(clippy::needless_pass_by_ref_mut)]
pub fn matching_indices<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &mut [Word], // check-added-lines: allow(index) slice type
    object: Word,
    options: SelectionOptions,
) -> Result<Vec<usize>, ObjectError> {
    scope_roots(ctx, values, |ctx, roots| {
        let option_roots = [
            object,
            options.key.unwrap_or(Word::NIL),
            options.test.unwrap_or(Word::NIL),
            options.test_not.unwrap_or(Word::NIL),
        ];
        scope_roots(ctx, &option_roots, |ctx, option_roots| {
            let rooted_options = || -> Result<SelectionOptions, ObjectError> {
                let key = option_roots.get(1).ok_or(ObjectError::Layout).copied()?;
                let test = option_roots.get(2).ok_or(ObjectError::Layout).copied()?;
                let test_not = option_roots.get(3).ok_or(ObjectError::Layout).copied()?;
                Ok(SelectionOptions {
                    key: (key != Word::NIL).then_some(key),
                    test: (test != Word::NIL).then_some(test),
                    test_not: (test_not != Word::NIL).then_some(test_not),
                    ..options
                })
            };
            let (start, end) = bounds(options, values.len())?;
            if options.count == Some(0) {
                return Ok(Vec::new());
            }
            let mut indices = (start..end).collect::<Vec<_>>();
            if options.from_end {
                indices.reverse();
            }
            let mut found = Vec::new();
            for index in indices {
                if matches(
                    caller,
                    ctx,
                    runtime,
                    rooted_options()?,
                    *roots.get(index).ok_or(ObjectError::Layout)?,
                    *option_roots.first().ok_or(ObjectError::Layout)?,
                )? {
                    found.push(index);
                    if let Some(count) = options.count
                        && found.len() >= count
                    {
                        break;
                    }
                }
            }
            Ok(found)
        })
    })
}

#[allow(clippy::needless_pass_by_ref_mut)]
pub fn find<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &mut [Word], // check-added-lines: allow(index) slice type
    object: Word,
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    let Some(index) = matching_indices(ctx, runtime, caller, values, object, options)?
        .first()
        .copied()
    else {
        return Ok(Word::NIL);
    };
    values.get(index).copied().ok_or(ObjectError::Layout)
}

pub fn position<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &mut [Word], // check-added-lines: allow(index) slice type
    object: Word,
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    let index = matching_indices(ctx, runtime, caller, values, object, options)?
        .first()
        .copied();
    index.map_or(Ok(Word::NIL), |index| {
        i64::try_from(index)
            .map(Word::fixnum)
            .map_err(|_| ObjectError::TypeError)
    })
}

pub fn count<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &mut [Word], // check-added-lines: allow(index) slice type
    object: Word,
    mut options: SelectionOptions,
) -> Result<Word, ObjectError> {
    options.count = None;
    let count =
        i64::try_from(matching_indices(ctx, runtime, caller, values, object, options)?.len())
            .map_err(|_| ObjectError::TypeError)?;
    Ok(Word::fixnum(count))
}

pub fn remove<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &mut [Word], // check-added-lines: allow(index) slice type
    object: Word,
    options: SelectionOptions,
) -> Result<Vec<Word>, ObjectError> {
    let indices = matching_indices(ctx, runtime, caller, values, object, options)?;
    let removed = indices
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    Ok(values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (!removed.contains(&index)).then_some(*value))
        .collect())
}

pub fn substitute<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &mut [Word], // check-added-lines: allow(index) slice type
    replacement: Word,
    object: Word,
    options: SelectionOptions,
) -> Result<Vec<Word>, ObjectError> {
    let indices = matching_indices(ctx, runtime, caller, values, object, options)?;
    let replaced = indices
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    Ok(values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if replaced.contains(&index) {
                replacement
            } else {
                *value
            }
        })
        .collect())
}

#[path = "selection_if.rs"]
mod selection_if;
pub use selection_if::{count_if, find_if, position_if, remove_if, substitute_if};

#[path = "selection_duplicates.rs"]
mod selection_duplicates;
pub use selection_duplicates::remove_duplicates;

#[path = "selection_search.rs"]
mod selection_search;
#[allow(unused_imports)]
pub use selection_search::{mismatch, search};
pub use selection_search::{mismatch_pair, search_pair};
