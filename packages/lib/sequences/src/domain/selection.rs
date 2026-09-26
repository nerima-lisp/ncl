//! Common Lisp selection and matching operations.

#![allow(dead_code)]
//!
//! The implementation keeps a rooted snapshot of a sequence while calling
//! Lisp functions.  This is important: a key or predicate may allocate and
//! move every object in the sequence.

use ncl_object::{
    FunctionArguments, FunctionCaller, FunctionDesignator, List, MultipleValues, ObjectError,
    ObjectRef, Runtime, Sequence, ThreadContext, Word, car, cdr, classify_object, make_cons,
    pop_root, push_root, simple_vector_length, simple_vector_ref, string_length, string_ref,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SelectionOptions {
    pub start: usize,
    pub end: Option<usize>,
    pub from_end: bool,
    pub count: Option<usize>,
    pub key: Option<Word>,
    pub test: Option<Word>,
    pub test_not: Option<Word>,
}

pub fn parse_options(
    ctx: &ThreadContext,
    args: &[Word],
) -> Result<(Vec<Word>, SelectionOptions), ObjectError> {
    let mut positional = Vec::new();
    let mut options = SelectionOptions::default();
    let mut index = 0;
    while index < args.len() {
        let name = if let ObjectRef::Symbol(symbol) = classify_object(ctx, args[index]) {
            let name = ncl_object::symbol_name(ctx, symbol)?;
            (0..ncl_object::string_length(ctx, name)?)
                .map(|i| ncl_object::string_ref(ctx, name, i))
                .collect::<Result<String, _>>()?
        } else {
            String::new()
        };
        let keyword = name.to_ascii_uppercase();
        if matches!(
            keyword.as_str(),
            "START" | "END" | "FROM-END" | "COUNT" | "KEY" | "TEST" | "TEST-NOT"
        ) {
            let value = *args.get(index + 1).ok_or(ObjectError::TypeError)?;
            if keyword == "START" {
                options.start = usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                    .map_err(|_| ObjectError::TypeError)?;
            } else if keyword == "END" {
                options.end = Some(
                    usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                        .map_err(|_| ObjectError::TypeError)?,
                );
            } else if keyword == "FROM-END" {
                options.from_end = value != Word::NIL;
            } else if keyword == "COUNT" {
                options.count = Some(
                    usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                        .map_err(|_| ObjectError::TypeError)?,
                );
            } else if keyword == "KEY" {
                options.key = Some(value);
            } else if keyword == "TEST" {
                options.test = Some(value);
            } else if keyword == "TEST-NOT" {
                options.test_not = Some(value);
            }
            index += 2;
        } else {
            positional.push(args[index]);
            index += 1;
        }
    }
    if positional.len() < 2 || options.test.is_some() && options.test_not.is_some() {
        return Err(ObjectError::TypeError);
    }
    Ok((positional, options))
}

pub fn sequence_values(
    ctx: &mut ThreadContext,
    sequence: Sequence,
) -> Result<Vec<Word>, ObjectError> {
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
    ncl_object::with_roots(ctx, &roots, |ctx, roots| {
        let mut values = MultipleValues::new();
        caller.call_function(
            ctx,
            runtime,
            function(ctx, **roots.first().ok_or(ObjectError::Layout)?)?,
            FunctionArguments::new(&[**roots.get(1).ok_or(ObjectError::Layout)?]),
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
    ncl_object::with_roots(ctx, &predicate_roots, |ctx, roots| {
        let keyed = **roots.first().ok_or(ObjectError::Layout)?;
        let object = **roots.get(1).ok_or(ObjectError::Layout)?;
        let result = if options.test.is_some() {
            let designator = function(ctx, **roots.get(3).ok_or(ObjectError::Layout)?)?;
            let mut values = MultipleValues::new();
            let args = [keyed, object];
            caller.call_function(
                ctx,
                runtime,
                designator,
                FunctionArguments::new(&args),
                &mut values,
            )?
        } else if options.test_not.is_some() {
            let designator = function(ctx, **roots.get(4).ok_or(ObjectError::Layout)?)?;
            let mut values = MultipleValues::new();
            let args = [keyed, object];
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
    let end = options.end.unwrap_or(length);
    if options.start > end || end > length {
        Err(ObjectError::TypeError)
    } else {
        Ok((options.start, end))
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
    values: &mut [Word],
    object: Word,
    options: SelectionOptions,
) -> Result<Vec<usize>, ObjectError> {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let option_roots = [
            object,
            options.key.unwrap_or(Word::NIL),
            options.test.unwrap_or(Word::NIL),
            options.test_not.unwrap_or(Word::NIL),
        ];
        ncl_object::with_roots(ctx, &option_roots, |ctx, option_roots| {
            let rooted_options = || -> Result<SelectionOptions, ObjectError> {
                let key = option_roots
                    .get(1)
                    .ok_or(ObjectError::Layout)
                    .map(|r| **r)?;
                let test = option_roots
                    .get(2)
                    .ok_or(ObjectError::Layout)
                    .map(|r| **r)?;
                let test_not = option_roots
                    .get(3)
                    .ok_or(ObjectError::Layout)
                    .map(|r| **r)?;
                Ok(SelectionOptions {
                    key: (key != Word::NIL).then_some(key),
                    test: (test != Word::NIL).then_some(test),
                    test_not: (test_not != Word::NIL).then_some(test_not),
                    ..options
                })
            };
            let (start, end) = bounds(options, values.len())?;
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
                    **roots.get(index).ok_or(ObjectError::Layout)?,
                    **option_roots.first().ok_or(ObjectError::Layout)?,
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
    values: &mut [Word],
    object: Word,
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    Ok(
        matching_indices(ctx, runtime, caller, values, object, options)?
            .first()
            .map_or(Word::NIL, |&index| values[index]),
    )
}

pub fn position<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &mut [Word],
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
    values: &mut [Word],
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
    values: &mut [Word],
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
    values: &mut [Word],
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

#[allow(clippy::needless_pass_by_ref_mut)]
pub fn search<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    left: &mut [Word],
    right: &mut [Word],
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, left, |ctx, left_roots| {
        ncl_object::with_roots(ctx, right, |ctx, right_roots| {
            let left_snapshot = left_roots.iter().map(|root| **root).collect::<Vec<_>>();
            let right_snapshot = right_roots.iter().map(|root| **root).collect::<Vec<_>>();
            let (start, end) = bounds(options, left.len())?;
            let width = end - start;
            if width > right.len() {
                return Ok(Word::NIL);
            }
            let candidates: Vec<usize> = if options.from_end {
                (0..=right.len() - width).rev().collect()
            } else {
                (0..=right.len() - width).collect()
            };
            for offset in candidates {
                let mut equal = true;
                for i in 0..width {
                    if !matches(
                        caller,
                        ctx,
                        runtime,
                        options,
                        left_snapshot[start + i],
                        right_snapshot[offset + i],
                    )? {
                        equal = false;
                        break;
                    }
                }
                if equal {
                    return index_word(offset);
                }
            }
            Ok(Word::NIL)
        })
    })
}

#[allow(clippy::needless_pass_by_ref_mut)]
pub fn mismatch<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    left: &mut [Word],
    right: &mut [Word],
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, left, |ctx, left_roots| {
        ncl_object::with_roots(ctx, right, |ctx, right_roots| {
            let left_snapshot = left_roots.iter().map(|root| **root).collect::<Vec<_>>();
            let right_snapshot = right_roots.iter().map(|root| **root).collect::<Vec<_>>();
            let (start, end) = bounds(options, left.len())?;
            for index in start..end.min(right.len()) {
                if !matches(
                    caller,
                    ctx,
                    runtime,
                    options,
                    left_snapshot[index],
                    right_snapshot[index],
                )? {
                    return index_word(index);
                }
            }
            if end - start == right.len().saturating_sub(start) {
                Ok(Word::NIL)
            } else {
                index_word(end.min(right.len()))
            }
        })
    })
}

#[allow(clippy::needless_pass_by_ref_mut)]
pub fn list_from_values(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &mut [Word],
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let mut result = Word::NIL;
        let result_root = push_root(ctx, &mut result);
        for root in roots.iter().rev() {
            result = make_cons(ctx, runtime, **root, result)?;
        }
        if pop_root(ctx, result_root) {
            Ok(result)
        } else {
            Err(ObjectError::Layout)
        }
    })
}

pub fn object_sequence(ctx: &ThreadContext, word: Word) -> Result<Sequence, ObjectError> {
    if word == Word::NIL {
        return Ok(Sequence::List(List::Nil));
    }
    if word.is_cons() {
        return Ok(Sequence::List(List::Cons(ncl_object::Cons::from_word(
            word,
        ))));
    }
    if let ObjectRef::String(value) = classify_object(ctx, word) {
        Ok(Sequence::String(ncl_object::StringObject::from_word(value)))
    } else if let ObjectRef::SimpleVector(value) = classify_object(ctx, word) {
        Ok(Sequence::Vector(ncl_object::SimpleVector::from_word(value)))
    } else {
        Err(ObjectError::TypeError)
    }
}
