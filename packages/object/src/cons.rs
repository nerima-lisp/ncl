use crate::{ObjectError, ThreadContext};
use ncl_sys::{StorageCondition, Word};

/// Replace the car of a cons cell.
///
/// # Errors
///
/// Returns [`ObjectError`] when the value is not a mutable cons cell.
pub fn rplaca(ctx: &mut ThreadContext, word: Word, value: Word) -> Result<Word, ObjectError> {
    ctx.check_registered_address()?;
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
    ctx.check_registered_address()?;
    if !word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    if !ncl_sys::write_cons_word(&mut ctx.thread, word, 1, value) {
        return Err(ObjectError::Storage(StorageCondition::ThreadNotRegistered));
    }
    ncl_sys::write_barrier(&mut ctx.thread, word, 1);
    Ok(word)
}
