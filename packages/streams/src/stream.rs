use ncl_object::{
    ObjectError, Runtime, ThreadContext, Word, allocate, get, put, stream_offset, widetag,
    with_roots,
};

/// A typed handle to a Lisp stream object.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stream(Word);

impl Stream {
    /// Construct a stream handle from its tagged word.
    #[must_use]
    pub const fn from_word(value: Word) -> Self {
        Self(value)
    }

    /// Return the tagged word represented by this handle.
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.0
    }
}

impl From<Stream> for Word {
    fn from(value: Stream) -> Self {
        value.0
    }
}

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
