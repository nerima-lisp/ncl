use crate::character::{
    StreamKind, ensure_open, position, set_position, state_kind, stream_from_args,
};
use crate::{CLOSED, DATA, POSITION};
use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, Stream, ThreadContext, Word,
    classify_object, simple_vector_ref, simple_vector_set, stream_element_type,
    stream_external_format, stream_state,
};

mod adapters;
mod core;
mod io;

pub(crate) use adapters::*;
pub(crate) use core::*;
pub(crate) use io::*;
