//! `REDUCE`, split out of `higher_order.rs` to keep that file under the
//! project's line-count limit.

use ncl_object::{
    FunctionCaller, FunctionDesignator, ObjectError, Runtime, Sequence, ThreadContext, Word,
};

use super::{call, callback, scope_rooted_slice, scope_roots, values};

/// Options accepted by REDUCE beyond the function and sequence.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReduceOptions {
    pub key: Option<Word>,
    pub initial_value: Option<Word>,
    pub from_end: bool,
    pub start: usize,
    pub end: Option<usize>,
}

/// REDUCE with `:key`, `:initial-value`, `:from-end`, `:start`, and `:end`.
pub fn reduce<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    sequence: Sequence,
    options: ReduceOptions,
    caller: &mut C,
) -> Result<Word, ObjectError> {
    let ReduceOptions {
        key,
        initial_value: initial,
        from_end,
        start,
        end,
    } = options;
    let roots = [
        function,
        initial.unwrap_or(Word::NIL),
        key.unwrap_or(Word::NIL),
    ];
    scope_roots(ctx, &roots, |ctx, roots| {
        let all_items = values(ctx, sequence)?;
        let end = end.unwrap_or(all_items.len());
        if start > end || end > all_items.len() {
            return Err(ObjectError::TypeError);
        }
        let mut items = all_items
            .get(start..end)
            .ok_or(ObjectError::Layout)?
            .to_vec();
        if from_end {
            items.reverse();
        }
        let function_callback = callback(ctx, *roots.first().ok_or(ObjectError::Layout)?)?;
        if items.is_empty() && initial.is_none() {
            // CLHS 17.2.1: an empty subsequence with no :initial-value calls
            // the reducing function with zero arguments.
            return Ok(call(ctx, runtime, caller, function_callback, &[])?.0);
        }
        let key_callback = key
            .map(|_| callback(ctx, *roots.get(2).ok_or(ObjectError::Layout)?))
            .transpose()?;
        fn apply_key<C: FunctionCaller>(
            ctx: &mut ThreadContext,
            runtime: &Runtime,
            caller: &mut C,
            key_callback: Option<FunctionDesignator>,
            value: Word,
        ) -> Result<Word, ObjectError> {
            match key_callback {
                Some(key_callback) => Ok(call(ctx, runtime, caller, key_callback, &[value])?.0),
                None => Ok(value),
            }
        }
        scope_rooted_slice(ctx, &items, |ctx, items| {
            let mut index = 0;
            let accumulator = if initial.is_some() {
                *roots.get(1).ok_or(ObjectError::Layout)?
            } else {
                let first = *items.first().ok_or(ObjectError::TypeError)?;
                index = 1;
                apply_key(ctx, runtime, caller, key_callback, first)?
            };
            let accumulator_root = [accumulator];
            scope_rooted_slice(ctx, &accumulator_root, |ctx, accumulator| {
                while index < items.len() {
                    let element = apply_key(
                        ctx,
                        runtime,
                        caller,
                        key_callback,
                        *items.get(index).ok_or(ObjectError::Layout)?,
                    )?;
                    let (left, right) = if from_end {
                        (element, *accumulator.first().ok_or(ObjectError::Layout)?)
                    } else {
                        (*accumulator.first().ok_or(ObjectError::Layout)?, element)
                    };
                    let args = [left, right];
                    *accumulator.first_mut().ok_or(ObjectError::Layout)? =
                        call(ctx, runtime, caller, function_callback, &args)?.0;
                    index += 1;
                }
                Ok(*accumulator.first().ok_or(ObjectError::Layout)?)
            })
        })
    })
}
