use ncl_object::array::{
    adjust_array, adjustable_array_p, array_element_type, array_has_fill_pointer_p, fill_pointer,
};
use ncl_object::{
    ArrayOptions, BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word,
    array_row_major_ref, array_row_major_set, make_array, pop_root, push_root,
};

use super::helpers::{array_shape, list_values};
use super::symbol_text;

pub(super) fn adjust_array_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let dimensions = list_values(ctx, args.required(1)?)?
        .into_iter()
        .map(|value| {
            usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                .map_err(|_| ObjectError::TypeError)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let object = args.required(0)?;
    let mut initial = Word::NIL;
    let mut fill_pointer_value = None;
    let mut displaced_to = None;
    let mut displaced_index_offset = 0;
    let options = &args.as_slice()[2..];
    if !options.len().is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    for pair in options.as_chunks::<2>().0 {
        match symbol_text(ctx, pair[0])?
            .to_ascii_uppercase()
            .trim_start_matches(':')
        {
            "INITIAL-ELEMENT" => initial = pair[1],
            "FILL-POINTER" => {
                fill_pointer_value = Some(
                    usize::try_from(pair[1].as_fixnum().ok_or(ObjectError::TypeError)?)
                        .map_err(|_| ObjectError::TypeError)?,
                );
            }
            "DISPLACED-TO" => displaced_to = (pair[1] != Word::NIL).then_some(pair[1]),
            "DISPLACED-INDEX-OFFSET" => {
                displaced_index_offset =
                    usize::try_from(pair[1].as_fixnum().ok_or(ObjectError::TypeError)?)
                        .map_err(|_| ObjectError::TypeError)?;
            }
            _ => return Err(ObjectError::TypeError),
        }
    }
    if displaced_to.is_none() && fill_pointer_value.is_none() {
        return adjust_array(ctx, runtime, object, &dimensions, initial);
    }
    if !adjustable_array_p(ctx, object)? {
        return Err(ObjectError::TypeError);
    }
    adjust_array_with_options(
        ctx,
        runtime,
        object,
        AdjustArrayOptions {
            dimensions: &dimensions,
            initial,
            fill_pointer: fill_pointer_value,
            displaced_to,
            displaced_index_offset,
        },
    )
}

#[derive(Clone, Copy)]
struct AdjustArrayOptions<'a> {
    dimensions: &'a [usize],
    initial: Word,
    fill_pointer: Option<usize>,
    displaced_to: Option<Word>,
    displaced_index_offset: usize,
}

fn adjust_array_with_options(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
    options: AdjustArrayOptions<'_>,
) -> Result<Word, ObjectError> {
    let AdjustArrayOptions {
        dimensions,
        initial,
        fill_pointer: requested_fill_pointer,
        displaced_to,
        displaced_index_offset,
    } = options;
    let mut rooted_object = object;
    let object_token = push_root(ctx, &mut rooted_object);
    let mut rooted_initial = initial;
    let initial_token = push_root(ctx, &mut rooted_initial);
    let mut rooted_displaced = displaced_to.unwrap_or(Word::NIL);
    let displaced_token = displaced_to.map(|_| push_root(ctx, &mut rooted_displaced));
    let result = (|| {
        let object = rooted_object;
        let initial = rooted_initial;
        let displaced_to = displaced_to.map(|_| rooted_displaced);
        let element_type = array_element_type(ctx, object)?;
        let had_fill_pointer = array_has_fill_pointer_p(ctx, object)?;
        if requested_fill_pointer.is_some() && dimensions.len() != 1 {
            return Err(ObjectError::TypeError);
        }
        let result = make_array(
            ctx,
            runtime,
            dimensions,
            ArrayOptions {
                element_type,
                initial_element: initial,
                adjustable: true,
                fill_pointer: requested_fill_pointer.or_else(|| {
                    had_fill_pointer
                        .then(|| fill_pointer(ctx, object).ok())
                        .flatten()
                }),
                displaced_to,
                displaced_index_offset,
            },
        )?;
        let mut rooted_result = result;
        let result_token = push_root(ctx, &mut rooted_result);
        let copy_result = (|| {
            if displaced_to.is_none() {
                let old_dimensions = array_shape(ctx, rooted_object)?;
                let old_total = old_dimensions
                    .iter()
                    .try_fold(1usize, |size, dimension| size.checked_mul(*dimension))
                    .ok_or(ObjectError::Layout)?;
                let new_total = dimensions
                    .iter()
                    .try_fold(1usize, |size, dimension| size.checked_mul(*dimension))
                    .ok_or(ObjectError::Layout)?;
                for index in 0..old_total.min(new_total) {
                    let value = array_row_major_ref(ctx, rooted_object, index)?;
                    array_row_major_set(ctx, rooted_result, index, value)?;
                }
            }
            Ok(rooted_result)
        })();
        if !pop_root(ctx, result_token) {
            return Err(ObjectError::Layout);
        }
        copy_result
    })();
    let displaced_popped = displaced_token.is_none_or(|token| pop_root(ctx, token));
    let initial_popped = pop_root(ctx, initial_token);
    let object_popped = pop_root(ctx, object_token);
    if !displaced_popped || !initial_popped || !object_popped {
        return Err(ObjectError::Layout);
    }
    result
}
