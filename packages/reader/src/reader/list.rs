//! Reading a parenthesized list, including the dotted-pair tail form.

use std::cell::Cell;

use ncl_object::{Runtime, ThreadContext, Word, make_cons, pop_root, push_root, rplacd};

use crate::error::ReadError;
use crate::input::CharSource;

use super::{ReadOptions, read_form, skip_whitespace};

/// Read a proper list terminated by `)`, with optional dotted-pair support.
pub fn read_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &Cell<Word>,
    labels: &Cell<Word>,
) -> Result<Word, ReadError> {
    let mut head = Cell::new(Word::NIL);
    let mut tail = Cell::new(Word::NIL);
    let head_token = push_root(ctx, head.get_mut());
    let tail_token = push_root(ctx, tail.get_mut());
    let result = read_list_inner(ctx, runtime, source, opts, rt, labels, &head, &tail);
    let _ = pop_root(ctx, tail_token);
    let _ = pop_root(ctx, head_token);
    result
}

/// The loop body of [`read_list`]; `head` and `tail` are caller-rooted slots.
fn read_list_inner(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &Cell<Word>,
    labels: &Cell<Word>,
    head: &Cell<Word>,
    tail: &Cell<Word>,
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
        let Some(form) = form else {
            return Err(ReadError::UnexpectedEof);
        };
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
