use crate::{CLOSED, DATA, FILE_OUTPUT, POSITION, STRING_INPUT, STRING_OUTPUT};
use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Package, Runtime, Stream, ThreadContext,
    Word, car, cdr, classify_object, make_cons, make_simple_vector, make_stream, make_string,
    pop_root, push_root, simple_vector_length, simple_vector_ref, simple_vector_set, stream_state,
    string_length, string_ref, with_root,
};

mod adapters;
mod input;
mod output;
mod state;

pub(crate) use adapters::*;
pub(crate) use input::*;
pub(crate) use output::*;
pub(crate) use state::*;

/// The predefined Common Lisp class represented by a stream implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamClass {
    /// A stream implementation without a more specific class mapping.
    Generic,
    /// A file stream.
    File,
    /// A string stream.
    String,
    /// A two-way stream.
    TwoWay,
}

/// Classify a stream from its implementation state.
///
/// # Errors
/// Returns an object-layer error when the stream or its state is malformed.
pub fn stream_class(ctx: &ThreadContext, stream: Stream) -> Result<StreamClass, ObjectError> {
    let kind = state_kind(ctx, stream_state(ctx, stream)?)?;
    Ok(match kind {
        StreamKind::StringInput | StreamKind::StringOutput => StreamClass::String,
        StreamKind::FileOutput | StreamKind::FileIo => StreamClass::File,
        StreamKind::Data
        | StreamKind::Closed
        | StreamKind::StandardInput
        | StreamKind::StandardOutput
        | StreamKind::StandardError => StreamClass::Generic,
        StreamKind::StandardTwoWay => StreamClass::TwoWay,
    })
}
