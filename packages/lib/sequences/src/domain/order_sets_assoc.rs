use super::{Options, matches};
use ncl_object::{ObjectError, Runtime, ThreadContext, Word, car, cdr};

fn assoc_like(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    alist: Word,
    opts: Options,
    reverse: bool,
) -> Result<Word, ObjectError> {
    let mut cursor = alist;
    while cursor != Word::NIL {
        if !cursor.is_cons() {
            return Err(ObjectError::TypeError);
        }
        let pair = car(ctx, cursor)?;
        if !pair.is_cons() {
            return Err(ObjectError::TypeError);
        }
        let value = if reverse {
            cdr(ctx, pair)?
        } else {
            car(ctx, pair)?
        };
        if matches(ctx, runtime, item, value, opts)? {
            return Ok(pair);
        }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
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
    let mut cursor = list;
    while cursor != Word::NIL {
        if !cursor.is_cons() {
            return Err(ObjectError::TypeError);
        }
        let value = car(ctx, cursor)?;
        if matches(ctx, runtime, item, value, opts)? {
            return Ok(cursor);
        }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
}
