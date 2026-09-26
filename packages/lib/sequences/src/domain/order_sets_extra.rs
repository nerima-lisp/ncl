use super::{
    Options, list_from, matches, rooted_nested, sequence_values, set_operation, with_options,
};
use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

pub fn intersection(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    first: Word,
    second: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    let mut rows = vec![sequence_values(ctx, first)?, sequence_values(ctx, second)?];
    rooted_nested(ctx, &mut rows, |ctx, rows| {
        with_options(ctx, opts, |ctx, opts| {
            let result = vec![Word::NIL; rows.first().map_or(0, Vec::len)];
            ncl_object::with_rooted_slice(ctx, &result, |ctx, result| {
                let mut result_len = 0;
                for value in rows.first().ok_or(ObjectError::Layout)? {
                    let mut found = false;
                    for candidate in rows.get(1).ok_or(ObjectError::Layout)? {
                        if matches(ctx, runtime, *value, *candidate, opts)? {
                            found = true;
                            break;
                        }
                    }
                    let mut duplicate = false;
                    for candidate in result.iter().take(result_len) {
                        if matches(ctx, runtime, *value, *candidate, opts)? {
                            duplicate = true;
                            break;
                        }
                    }
                    if found && !duplicate {
                        *result.get_mut(result_len).ok_or(ObjectError::Layout)? = *value;
                        result_len += 1;
                    }
                }
                list_from(
                    ctx,
                    runtime,
                    result.get(..result_len).ok_or(ObjectError::Layout)?,
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
    let mut rows = vec![sequence_values(ctx, first)?, sequence_values(ctx, second)?];
    rooted_nested(ctx, &mut rows, |ctx, rows| {
        with_options(ctx, opts, |ctx, opts| {
            for value in rows.first().ok_or(ObjectError::Layout)? {
                let mut found = false;
                for candidate in rows.get(1).ok_or(ObjectError::Layout)? {
                    if matches(ctx, runtime, *value, *candidate, opts)? {
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
    let mut rows = vec![vec![item], sequence_values(ctx, list)?];
    rooted_nested(ctx, &mut rows, |ctx, rows| {
        with_options(ctx, opts, |ctx, opts| {
            let item = *rows
                .first()
                .and_then(|row| row.first())
                .ok_or(ObjectError::Layout)?;
            let mut found = false;
            for value in rows.get(1).ok_or(ObjectError::Layout)? {
                if matches(ctx, runtime, item, *value, opts)? {
                    found = true;
                    break;
                }
            }
            if found {
                return Ok(list);
            }
            let mut values = Vec::with_capacity(rows.get(1).map_or(0, Vec::len) + 1);
            values.push(item);
            values.extend(rows.get(1).ok_or(ObjectError::Layout)?.iter().copied());
            ncl_object::with_rooted_slice(ctx, &values, |ctx, values| {
                list_from(ctx, runtime, values)
            })
        })
    })
}
