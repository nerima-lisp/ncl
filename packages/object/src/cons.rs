use crate::{ObjectError, ThreadContext};

use ncl_sys::{StorageCondition, Word};

/// Push a precise root.
pub fn push_root(ctx: &mut ThreadContext, value: &mut Word) -> ncl_sys::RootToken {
    ncl_sys::push_root(&mut ctx.thread, value)
}

/// Pop a precise root.
pub fn pop_root(ctx: &mut ThreadContext, token: ncl_sys::RootToken) -> bool {
    ncl_sys::pop_root(&mut ctx.thread, token)
}

/// Return the car of a cons cell.
///
/// # Errors
///
/// Returns [`ObjectError`] when the value is not a cons cell.
pub fn car(ctx: &ThreadContext, word: Word) -> Result<Word, ObjectError> {
    if word == Word::NIL {
        return Ok(Word::NIL);
    }
    if !word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::read_cons_word(&ctx.thread, word, 0)
        .ok_or(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
}

/// Return the cdr of a cons cell.
///
/// # Errors
///
/// Returns [`ObjectError`] when the value is not a cons cell.
pub fn cdr(ctx: &ThreadContext, word: Word) -> Result<Word, ObjectError> {
    if word == Word::NIL {
        return Ok(Word::NIL);
    }
    if !word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::read_cons_word(&ctx.thread, word, 1)
        .ok_or(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
}

/// Replace the car of a cons cell.
///
/// # Errors
///
/// Returns [`ObjectError`] when the value is not a mutable cons cell.
pub fn rplaca(ctx: &mut ThreadContext, word: Word, value: Word) -> Result<Word, ObjectError> {
    if !word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    if !ncl_sys::write_cons_word(&mut ctx.thread, word, 0, value) {
        return Err(ObjectError::Storage(StorageCondition::ThreadNotRegistered));
    }
    ncl_sys::write_barrier(&mut ctx.thread, word, 0);
    Ok(word)
}

/// Replace the cdr of a cons cell.
///
/// # Errors
///
/// Returns [`ObjectError`] when the value is not a mutable cons cell.
pub fn rplacd(ctx: &mut ThreadContext, word: Word, value: Word) -> Result<Word, ObjectError> {
    if !word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    if !ncl_sys::write_cons_word(&mut ctx.thread, word, 1, value) {
        return Err(ObjectError::Storage(StorageCondition::ThreadNotRegistered));
    }
    ncl_sys::write_barrier(&mut ctx.thread, word, 1);
    Ok(word)
}
