use super::{
    Options, list_from, matches, options_from_roots, row_len, row_value, scoped_rows,
    sequence_values, set_operation, with_options,
};
use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

pub fn intersection(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    first: Word,
    second: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    let rows = vec![sequence_values(ctx, first)?, sequence_values(ctx, second)?];
    scoped_rows(ctx, &rows, |ctx, row_roots, row_ranges| {
        with_options(ctx, opts, |ctx, option_roots| {
            let result = vec![Word::NIL; row_len(row_ranges, 0)?];
            ncl_object::with_rooted_slice(ctx, &result, |ctx, rooted_result| {
                let mut result_len = 0;
                for value_index in 0..row_len(row_ranges, 0)? {
                    let mut found = false;
                    for candidate_index in 0..row_len(row_ranges, 1)? {
                        let value = row_value(row_roots, row_ranges, 0, value_index)?;
                        let candidate = row_value(row_roots, row_ranges, 1, candidate_index)?;
                        if matches(
                            ctx,
                            runtime,
                            value,
                            candidate,
                            options_from_roots(option_roots)?,
                        )? {
                            found = true;
                            break;
                        }
                    }
                    let mut duplicate = false;
                    for index in 0..result_len {
                        let value = row_value(row_roots, row_ranges, 0, value_index)?;
                        let candidate = *rooted_result.get(index).ok_or(ObjectError::Layout)?;
                        if matches(
                            ctx,
                            runtime,
                            value,
                            candidate,
                            options_from_roots(option_roots)?,
                        )? {
                            duplicate = true;
                            break;
                        }
                    }
                    if found && !duplicate {
                        *rooted_result
                            .get_mut(result_len)
                            .ok_or(ObjectError::Layout)? =
                            row_value(row_roots, row_ranges, 0, value_index)?;
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

pub fn set_difference(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    first: Word,
    second: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    set_operation(ctx, runtime, first, second, opts, false, true)
}

pub fn set_exclusive_or(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    first: Word,
    second: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    set_operation(ctx, runtime, first, second, opts, true, false)
}

pub fn subsetp(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    first: Word,
    second: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    let rows = vec![sequence_values(ctx, first)?, sequence_values(ctx, second)?];
    scoped_rows(ctx, &rows, |ctx, row_roots, row_ranges| {
        with_options(ctx, opts, |ctx, option_roots| {
            for value_index in 0..row_len(row_ranges, 0)? {
                let mut found = false;
                for candidate_index in 0..row_len(row_ranges, 1)? {
                    let value = row_value(row_roots, row_ranges, 0, value_index)?;
                    let candidate = row_value(row_roots, row_ranges, 1, candidate_index)?;
                    if matches(
                        ctx,
                        runtime,
                        value,
                        candidate,
                        options_from_roots(option_roots)?,
                    )? {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Ok(Word::NIL);
                }
            }
            Ok(Word::TRUE)
        })
    })
}

pub fn adjoin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    list: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    let list_root = [list];
    ncl_object::with_rooted_slice(ctx, &list_root, |ctx, list_root| {
        let rows = vec![
            vec![item],
            sequence_values(ctx, *list_root.first().ok_or(ObjectError::Layout)?)?,
        ];
        scoped_rows(ctx, &rows, |ctx, row_roots, row_ranges| {
            with_options(ctx, opts, |ctx, option_roots| {
                let item = row_value(row_roots, row_ranges, 0, 0)?;
                let mut found = false;
                for index in 0..row_len(row_ranges, 1)? {
                    let item = row_value(row_roots, row_ranges, 0, 0)?;
                    let value = row_value(row_roots, row_ranges, 1, index)?;
                    if matches(ctx, runtime, item, value, options_from_roots(option_roots)?)? {
                        found = true;
                        break;
                    }
                }
                if found {
                    return Ok(*list_root.first().ok_or(ObjectError::Layout)?);
                }
                let mut values = Vec::with_capacity(row_len(row_ranges, 1)? + 1);
                values.push(item);
                for index in 0..row_len(row_ranges, 1)? {
                    values.push(row_value(row_roots, row_ranges, 1, index)?);
                }
                list_from(ctx, runtime, &values)
            })
        })
    })
}
