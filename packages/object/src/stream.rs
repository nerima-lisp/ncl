use crate::object_access::{get, put};
use crate::{ObjectError, Runtime, ThreadContext, allocate, stream_offset, widetag, with_roots};
use ncl_sys::Word;

crate::word_newtype!(Stream);

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
    with_roots(
        ctx,
        &[
            direction,
            element_type,
            external_format,
            state,
            implementation,
        ],
        |ctx, values| {
            let object = allocate(ctx, runtime, widetag::STREAM, 5)?;
            for (i, v) in values.iter().copied().enumerate() {
                put(ctx, object, i, v)?;
            }
            Ok(object.into())
        },
    )
}
/// Read stream state.
///
/// # Errors
/// Returns an error when the object is invalid.
pub fn stream_state(ctx: &ThreadContext, object: Stream) -> Result<Word, ObjectError> {
    get(ctx, object.into(), widetag::STREAM, stream_offset::STATE)
}
/// Read a named stream slot.
///
/// # Errors
/// Returns an error when the object or slot is invalid.
pub fn stream_slot(ctx: &ThreadContext, object: Stream, slot: usize) -> Result<Word, ObjectError> {
    get(ctx, object.into(), widetag::STREAM, slot)
}

macro_rules! stream_accessor {
    ($name:ident, $slot:expr, $doc:literal) => {
        #[doc = $doc]
        ///
        /// # Errors
        /// Returns an error when the object is not a stream.
        pub fn $name(ctx: &ThreadContext, object: Stream) -> Result<Word, ObjectError> {
            stream_slot(ctx, object, $slot)
        }
    };
}
stream_accessor!(
    stream_direction,
    stream_offset::DIRECTION,
    "Read a stream direction."
);
stream_accessor!(
    stream_element_type,
    stream_offset::ELEMENT_TYPE,
    "Read a stream element type."
);
stream_accessor!(
    stream_external_format,
    stream_offset::EXTERNAL_FORMAT,
    "Read a stream external format."
);
stream_accessor!(
    stream_implementation,
    stream_offset::IMPLEMENTATION,
    "Read a stream implementation."
);
