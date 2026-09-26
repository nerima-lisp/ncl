use super::{Options, matches, with_scope};
use ncl_object::{Local, ObjectError, Runtime, ThreadContext, Word};

fn assoc_like(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    item: Word,
    alist: Word,
    opts: Options,
    reverse: bool,
) -> Result<Word, ObjectError> {
    let root_values = [item, alist, opts.key, opts.test, opts.test_not];
    with_scope(ctx, &root_values, |scope, handles| {
        let item = scope
            .get(*handles.iter().next().ok_or(ObjectError::Layout)?)
            .as_word();
        let cursor = scope.root(Local::from_word(
            scope
                .get(*handles.iter().nth(1).ok_or(ObjectError::Layout)?)
                .as_word(),
        ));
        while scope.get(cursor).as_word() != Word::NIL {
            if !scope.get(cursor).as_word().is_cons() {
                return Err(ObjectError::TypeError);
            }
            let pair = scope.root(Local::from_word(scope.car(cursor)?.as_word()));
            if !scope.get(pair).as_word().is_cons() {
                return Err(ObjectError::TypeError);
            }
            let value = if reverse {
                scope.cdr(pair)?.as_word()
            } else {
                scope.car(pair)?.as_word()
            };
            let options = Options {
                key: scope
                    .get(*handles.iter().nth(2).ok_or(ObjectError::Layout)?)
                    .as_word(),
                test: scope
                    .get(*handles.iter().nth(3).ok_or(ObjectError::Layout)?)
                    .as_word(),
                test_not: scope
                    .get(*handles.iter().nth(4).ok_or(ObjectError::Layout)?)
                    .as_word(),
            };
            if matches(scope.context_mut(), runtime, item, value, options)? {
                return Ok(scope.get(pair).as_word());
            }
            let next = scope.cdr(cursor)?.as_word();
            scope.set(cursor, Local::from_word(next));
        }
        Ok(Word::NIL)
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
    let root_values = [item, list, opts.key, opts.test, opts.test_not];
    with_scope(ctx, &root_values, |scope, handles| {
        let item = scope
            .get(*handles.iter().next().ok_or(ObjectError::Layout)?)
            .as_word();
        let cursor = scope.root(Local::from_word(
            scope
                .get(*handles.iter().nth(1).ok_or(ObjectError::Layout)?)
                .as_word(),
        ));
        while scope.get(cursor).as_word() != Word::NIL {
            if !scope.get(cursor).as_word().is_cons() {
                return Err(ObjectError::TypeError);
            }
            let value = scope.car(cursor)?.as_word();
            let options = Options {
                key: scope
                    .get(*handles.iter().nth(2).ok_or(ObjectError::Layout)?)
                    .as_word(),
                test: scope
                    .get(*handles.iter().nth(3).ok_or(ObjectError::Layout)?)
                    .as_word(),
                test_not: scope
                    .get(*handles.iter().nth(4).ok_or(ObjectError::Layout)?)
                    .as_word(),
            };
            if matches(scope.context_mut(), runtime, item, value, options)? {
                return Ok(scope.get(cursor).as_word());
            }
            let next = scope.cdr(cursor)?.as_word();
            scope.set(cursor, Local::from_word(next));
        }
        Ok(Word::NIL)
    })
}
