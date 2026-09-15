use crate::object_access::{get, put};
use crate::{ObjectError, Runtime, ThreadContext, allocate, widetag};
use ncl_sys::Word;

crate::word_newtype!(Readtable);

/// Allocate a readtable descriptor.
///
/// # Errors
/// Returns an error when allocation fails.
pub fn make_readtable(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    syntax: Word,
    dispatch: Word,
    case_mode: Word,
) -> Result<Readtable, ObjectError> {
    let object = allocate(ctx, runtime, widetag::READTABLE, 3)?;
    for (i, v) in [syntax, dispatch, case_mode].into_iter().enumerate() {
        put(ctx, object, i, v)?;
    }
    Ok(object.into())
}
/// Read a raw readtable slot.
///
/// # Errors
/// Returns an error when the object or slot is invalid.
pub fn readtable_slot(
    ctx: &ThreadContext,
    object: Readtable,
    slot: usize,
) -> Result<Word, ObjectError> {
    get(ctx, object.into(), widetag::READTABLE, slot)
}
