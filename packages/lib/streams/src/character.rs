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
