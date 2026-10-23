use super::{
    FunctionCaller, ObjectError, Runtime, SelectionOptions, ThreadContext, Word, bounds,
    index_word, matches, scope_roots,
};

#[allow(clippy::needless_pass_by_ref_mut)]
pub fn search<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    left: &mut [Word],  // check-added-lines: allow(index) slice type
    right: &mut [Word], // check-added-lines: allow(index) slice type
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    scope_roots(ctx, left, |ctx, left_roots| {
        scope_roots(ctx, right, |ctx, right_roots| {
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
                        *left_roots.get(start + i).ok_or(ObjectError::Layout)?,
                        *right_roots.get(offset + i).ok_or(ObjectError::Layout)?,
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
    scope_roots(ctx, left, |ctx, left_roots| {
        scope_roots(ctx, right, |ctx, right_roots| {
            let (start, end) = bounds(options, left.len())?;
            for index in start..end.min(right.len()) {
                if !matches(
                    caller,
                    ctx,
                    runtime,
                    options,
                    *left_roots.get(index).ok_or(ObjectError::Layout)?,
                    *right_roots.get(index).ok_or(ObjectError::Layout)?,
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
