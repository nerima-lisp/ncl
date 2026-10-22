use super::{PairSelectionOptions, SelectionOptions, bounds_from, call_one, function, scope_roots};
use ncl_object::{
    FunctionArguments, FunctionCaller, MultipleValues, ObjectError, Runtime, ThreadContext, Word,
};

const fn end1_bound(options: PairSelectionOptions) -> Option<usize> {
    match options.end1 {
        super::selection_options::End1Bound::Omitted => options.selection.end,
        super::selection_options::End1Bound::Unbounded => None,
        super::selection_options::End1Bound::Index(index) => Some(index),
    }
}

fn legacy_options(options: SelectionOptions) -> PairSelectionOptions {
    PairSelectionOptions {
        selection: options,
        ..PairSelectionOptions::default()
    }
}

fn matches_sequence_pair<C: FunctionCaller>(
    caller: &mut C,
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    options: PairSelectionOptions,
    left: Word,
    right: Word,
) -> Result<bool, ObjectError> {
    let has_key = options.selection.key.is_some_and(|key| key != Word::NIL);
    let has_test = options.selection.test.is_some();
    let has_test_not = options.selection.test_not.is_some();
    let roots = [
        left,
        right,
        options.selection.key.unwrap_or(Word::NIL),
        options.selection.test.unwrap_or(Word::NIL),
        options.selection.test_not.unwrap_or(Word::NIL),
    ];
    scope_roots(ctx, &roots, |ctx, roots| {
        let left = *roots.first().ok_or(ObjectError::Layout)?;
        let right = *roots.get(1).ok_or(ObjectError::Layout)?;
        let key = *roots.get(2).ok_or(ObjectError::Layout)?;
        let test = *roots.get(3).ok_or(ObjectError::Layout)?;
        let test_not = *roots.get(4).ok_or(ObjectError::Layout)?;
        let left_keyed = if has_key {
            call_one(caller, ctx, runtime, key, left)?
        } else {
            left
        };
        let keyed_roots = [left_keyed, right, key, test, test_not];
        scope_roots(ctx, &keyed_roots, |ctx, roots| {
            let left_keyed = *roots.first().ok_or(ObjectError::Layout)?;
            let right = *roots.get(1).ok_or(ObjectError::Layout)?;
            let key = *roots.get(2).ok_or(ObjectError::Layout)?;
            let test = *roots.get(3).ok_or(ObjectError::Layout)?;
            let test_not = *roots.get(4).ok_or(ObjectError::Layout)?;
            let right_keyed = if has_key {
                call_one(caller, ctx, runtime, key, right)?
            } else {
                right
            };
            let comparison_roots = [left_keyed, right_keyed, test, test_not];
            scope_roots(ctx, &comparison_roots, |ctx, roots| {
                let left = *roots.first().ok_or(ObjectError::Layout)?;
                let right = *roots.get(1).ok_or(ObjectError::Layout)?;
                let test = *roots.get(2).ok_or(ObjectError::Layout)?;
                let test_not = *roots.get(3).ok_or(ObjectError::Layout)?;
                let result = if has_test {
                    let mut values = MultipleValues::new();
                    caller.call_function(
                        ctx,
                        runtime,
                        function(ctx, test)?,
                        FunctionArguments::new(&[left, right]),
                        &mut values,
                    )?
                } else if has_test_not {
                    let mut values = MultipleValues::new();
                    caller.call_function(
                        ctx,
                        runtime,
                        function(ctx, test_not)?,
                        FunctionArguments::new(&[left, right]),
                        &mut values,
                    )?
                } else {
                    Word::NIL
                };
                Ok(if has_test_not {
                    result == Word::NIL
                } else if has_test {
                    result != Word::NIL
                } else {
                    left == right
                })
            })
        })
    })
}

fn index_word(index: usize) -> Result<Word, ObjectError> {
    i64::try_from(index)
        .map(Word::fixnum)
        .map_err(|_| ObjectError::TypeError)
}

#[allow(clippy::needless_pass_by_ref_mut)]
pub fn search<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    left: &mut [Word],  // check-added-lines: allow(index) slice type
    right: &mut [Word], // check-added-lines: allow(index) slice type
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    search_pair(ctx, runtime, caller, left, right, legacy_options(options))
}

#[allow(clippy::needless_pass_by_ref_mut)]
pub fn search_pair<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    left: &mut [Word],  // check-added-lines: allow(index) slice type
    right: &mut [Word], // check-added-lines: allow(index) slice type
    options: PairSelectionOptions,
) -> Result<Word, ObjectError> {
    scope_roots(ctx, left, |ctx, left_roots| {
        scope_roots(ctx, right, |ctx, right_roots| {
            let (start1, end1) = bounds_from(
                options.start1.unwrap_or(options.selection.start),
                end1_bound(options),
                left.len(),
            )?;
            let (start2, end2) =
                bounds_from(options.start2.unwrap_or(0), options.end2, right.len())?;
            let width = end1 - start1;
            if width > end2 - start2 {
                return Ok(Word::NIL);
            }
            let last_start = end2 - width;
            let candidates: Vec<usize> = if options.selection.from_end {
                (start2..=last_start).rev().collect()
            } else {
                (start2..=last_start).collect()
            };
            for offset in candidates {
                let mut equal = true;
                for index in 0..width {
                    if !matches_sequence_pair(
                        caller,
                        ctx,
                        runtime,
                        options,
                        *left_roots.get(start1 + index).ok_or(ObjectError::Layout)?,
                        *right_roots.get(offset + index).ok_or(ObjectError::Layout)?,
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
    left: &mut [Word],  // check-added-lines: allow(index) slice type
    right: &mut [Word], // check-added-lines: allow(index) slice type
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    mismatch_pair(ctx, runtime, caller, left, right, legacy_options(options))
}

#[allow(clippy::needless_pass_by_ref_mut)]
pub fn mismatch_pair<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    left: &mut [Word],  // check-added-lines: allow(index) slice type
    right: &mut [Word], // check-added-lines: allow(index) slice type
    options: PairSelectionOptions,
) -> Result<Word, ObjectError> {
    scope_roots(ctx, left, |ctx, left_roots| {
        scope_roots(ctx, right, |ctx, right_roots| {
            let (start1, end1) = bounds_from(
                options.start1.unwrap_or(options.selection.start),
                end1_bound(options),
                left.len(),
            )?;
            let (start2, end2) =
                bounds_from(options.start2.unwrap_or(0), options.end2, right.len())?;
            let left_length = end1 - start1;
            let right_length = end2 - start2;
            let compared = left_length.min(right_length);
            if options.selection.from_end {
                for offset in 0..compared {
                    let left_index = end1 - 1 - offset;
                    let right_index = end2 - 1 - offset;
                    if !matches_sequence_pair(
                        caller,
                        ctx,
                        runtime,
                        options,
                        *left_roots.get(left_index).ok_or(ObjectError::Layout)?,
                        *right_roots.get(right_index).ok_or(ObjectError::Layout)?,
                    )? {
                        return index_word(left_index + 1);
                    }
                }
                if left_length == right_length {
                    return Ok(Word::NIL);
                }
                return index_word(start1 + left_length.saturating_sub(right_length));
            }
            for offset in 0..compared {
                let left_index = start1 + offset;
                let right_index = start2 + offset;
                if !matches_sequence_pair(
                    caller,
                    ctx,
                    runtime,
                    options,
                    *left_roots.get(left_index).ok_or(ObjectError::Layout)?,
                    *right_roots.get(right_index).ok_or(ObjectError::Layout)?,
                )? {
                    return index_word(left_index);
                }
            }
            if left_length == right_length {
                Ok(Word::NIL)
            } else {
                index_word(start1 + compared)
            }
        })
    })
}
