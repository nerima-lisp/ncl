use super::{Options, matches};
use ncl_object::{ObjectError, Runtime, ThreadContext, Word, car, cdr, pop_root, push_root};

fn assoc_like(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    alist: Word,
    opts: Options,
    reverse: bool,
) -> Result<Word, ObjectError> {
    let rooted = [item, alist, opts.key, opts.test, opts.test_not];
    ncl_object::with_roots(ctx, &rooted, |ctx, rooted| {
        let mut cursor = **rooted.get(1).ok_or(ObjectError::Layout)?;
        let cursor_root = push_root(ctx, &mut cursor);
        let result = (|| {
            while cursor != Word::NIL {
                if !cursor.is_cons() {
                    return Err(ObjectError::TypeError);
                }
                let mut pair = car(ctx, cursor)?;
                let found = ncl_object::with_root(ctx, &mut pair, |ctx, pair| {
                    if !pair.is_cons() {
                        return Err(ObjectError::TypeError);
                    }
                    let value = if reverse {
                        cdr(ctx, *pair)?
                    } else {
                        car(ctx, *pair)?
                    };
                    let options = Options {
                        key: **rooted.get(2).ok_or(ObjectError::Layout)?,
                        test: **rooted.get(3).ok_or(ObjectError::Layout)?,
                        test_not: **rooted.get(4).ok_or(ObjectError::Layout)?,
                    };
                    if matches(
                        ctx,
                        runtime,
                        **rooted.first().ok_or(ObjectError::Layout)?,
                        value,
                        options,
                    )? {
                        return Ok(Some(*pair));
                    }
                    Ok(None)
                })?;
                if let Some(pair) = found {
                    return Ok(pair);
                }
                cursor = cdr(ctx, cursor)?;
            }
            Ok(Word::NIL)
        })();
        if pop_root(ctx, cursor_root) {
            result
        } else {
            Err(ObjectError::Layout)
        }
    })
}

pub fn assoc(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    alist: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    assoc_like(ctx, runtime, item, alist, opts, false)
}

pub fn rassoc(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    alist: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    assoc_like(ctx, runtime, item, alist, opts, true)
}

pub fn member(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    list: Word,
    opts: Options,
) -> Result<Word, ObjectError> {
    let rooted = [item, list, opts.key, opts.test, opts.test_not];
    ncl_object::with_roots(ctx, &rooted, |ctx, rooted| {
        let mut cursor = **rooted.get(1).ok_or(ObjectError::Layout)?;
        let cursor_root = push_root(ctx, &mut cursor);
        let result = (|| {
            while cursor != Word::NIL {
                if !cursor.is_cons() {
                    return Err(ObjectError::TypeError);
                }
                let value = car(ctx, cursor)?;
                let options = Options {
                    key: **rooted.get(2).ok_or(ObjectError::Layout)?,
                    test: **rooted.get(3).ok_or(ObjectError::Layout)?,
                    test_not: **rooted.get(4).ok_or(ObjectError::Layout)?,
                };
                if matches(
                    ctx,
                    runtime,
                    **rooted.first().ok_or(ObjectError::Layout)?,
                    value,
                    options,
                )? {
                    return Ok(cursor);
                }
                cursor = cdr(ctx, cursor)?;
            }
            Ok(Word::NIL)
        })();
        if pop_root(ctx, cursor_root) {
            result
        } else {
            Err(ObjectError::Layout)
        }
    })
}
