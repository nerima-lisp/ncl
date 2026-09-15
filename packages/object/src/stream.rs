use crate::object_access::{get, put};
use crate::{ObjectError, Runtime, ThreadContext, allocate, stream_offset, widetag};
use ncl_sys::Word;

pub type Stream = Word;

/// Allocate a stream descriptor.
///
/// # Errors
/// Returns an error when allocation fails.
pub fn make_stream(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    direction: Word,
    element_type: Word,
    external_format: Word,
    state: Word,
    implementation: Word,
) -> Result<Stream, ObjectError> {
    let object = allocate(ctx, runtime, widetag::STREAM, 5)?;
    for (i, v) in [
        direction,
        element_type,
        external_format,
        state,
        implementation,
    ]
    .into_iter()
    .enumerate()
    {
        put(ctx, object, i, v)?;
    }
    Ok(object)
}
/// Read stream state.
///
/// # Errors
/// Returns an error when the object is invalid.
pub fn stream_state(ctx: &ThreadContext, object: Stream) -> Result<Word, ObjectError> {
    get(ctx, object, widetag::STREAM, stream_offset::STATE)
}
/// Read a named stream slot.
///
/// # Errors
/// Returns an error when the object or slot is invalid.
pub fn stream_slot(ctx: &ThreadContext, object: Stream, slot: usize) -> Result<Word, ObjectError> {
    get(ctx, object, widetag::STREAM, slot)
}
