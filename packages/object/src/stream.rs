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
                put(ctx, object, i, *v)?;
            }
            Ok(Stream::from_word(object))
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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn stream_accessors_return_each_descriptor_field() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register");
        let stream = make_stream(
            &mut ctx,
            &runtime,
            Word::fixnum(1),
            Word::fixnum(2),
            Word::fixnum(3),
            Word::fixnum(4),
            Word::fixnum(5),
        )
        .expect("stream");
        assert_eq!(stream_direction(&ctx, stream), Ok(Word::fixnum(1)));
        assert_eq!(stream_element_type(&ctx, stream), Ok(Word::fixnum(2)));
        assert_eq!(stream_external_format(&ctx, stream), Ok(Word::fixnum(3)));
        assert_eq!(stream_state(&ctx, stream), Ok(Word::fixnum(4)));
        assert_eq!(stream_implementation(&ctx, stream), Ok(Word::fixnum(5)));
    }
}
