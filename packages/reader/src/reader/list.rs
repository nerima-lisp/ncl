use ncl_object::{Runtime, ThreadContext, Word, make_cons, rplacd};

use crate::error::ReadError;
use crate::input::CharSource;

use super::{ReadOptions, read_form, skip_whitespace};

pub fn read_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &ncl_object::HeapRootedWord,
    labels: &ncl_object::HeapRootedWord,
) -> Result<Word, ReadError> {
    let head = ncl_object::HeapRootedWord::new(ctx, Word::NIL)?;
    let tail = ncl_object::HeapRootedWord::new(ctx, Word::NIL)?;
    read_list_inner(ctx, runtime, source, opts, rt, labels, &head, &tail)
}

/// Parse list elements into the caller-rooted head and tail slots.
fn read_list_inner(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &ncl_object::HeapRootedWord,
    labels: &ncl_object::HeapRootedWord,
    head: &ncl_object::HeapRootedWord,
    tail: &ncl_object::HeapRootedWord,
) -> Result<Word, ReadError> {
    loop {
        skip_whitespace(ctx, source, opts, rt)?;
        let Some(c) = source.peek_char() else {
            return Err(ReadError::UnexpectedEof);
        };
        if c == ')' {
            source.read_char();
            break;
        }
        if c == '.' {
            source.read_char();
            let follower = source.peek_char();
            if follower.is_none_or(|n| n.is_whitespace() || n == ')') {
                if head.get() == Word::NIL {
                    return Err(ReadError::DotWithoutCdr);
                }
                let cdr = read_form(ctx, runtime, source, opts, rt, labels)?;
                let Some(cdr) = cdr else {
                    return Err(ReadError::DotWithoutCdr);
                };
                rplacd(ctx, tail.get(), cdr)?;
                skip_whitespace(ctx, source, opts, rt)?;
                if source.read_char() != Some(')') {
                    return Err(ReadError::UnmatchedRightParen);
                }
                break;
            }
            source.unread_char('.');
        }
        let form = read_form(ctx, runtime, source, opts, rt, labels)?;
        let Some(form) = form else { continue };
        let cell = make_cons(ctx, runtime, form, Word::NIL)?;
        if head.get() == Word::NIL {
            head.set(cell);
        } else {
            rplacd(ctx, tail.get(), cell)?;
        }
        tail.set(cell);
    }
    Ok(head.get())
}
