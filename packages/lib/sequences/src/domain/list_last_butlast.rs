use ncl_object::{LispError, Runtime, ThreadContext, Word, car as object_car, cdr as object_cdr};

pub(super) fn last_cons(ctx: &ThreadContext, mut cursor: Word) -> Result<Option<Word>, LispError> {
    let mut last = None;
    while cursor.is_cons() {
        last = Some(cursor);
        cursor = object_cdr(ctx, cursor)?;
    }
    Ok(last)
}

pub fn last(ctx: &ThreadContext, list: Word, count: usize) -> Result<Word, LispError> {
    let mut lead = list;
    for _ in 0..count {
        if !lead.is_cons() {
            return Ok(list);
        }
        lead = object_cdr(ctx, lead)?;
    }
    let mut result = list;
    while lead.is_cons() {
        lead = object_cdr(ctx, lead)?;
        result = object_cdr(ctx, result)?;
    }
    Ok(result)
}

pub fn butlast(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    list: Word,
    count: usize,
) -> Result<Word, LispError> {
    super::proper_list(ctx, list)?;

    let mut suffix = list;
    for _ in 0..count {
        if !suffix.is_cons() {
            return Ok(Word::NIL);
        }
        suffix = object_cdr(ctx, suffix)?;
    }
    let mut end = list;
    while suffix.is_cons() {
        suffix = object_cdr(ctx, suffix)?;
        end = object_cdr(ctx, end)?;
    }

    let mut items = Vec::new();
    let mut cursor = list;
    while cursor != end {
        items.push(object_car(ctx, cursor)?);
        cursor = object_cdr(ctx, cursor)?;
    }
    super::super::list_from(ctx, runtime, &items).map_err(LispError::from)
}
