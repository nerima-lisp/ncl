//! CLHS stream objects and their stored state.

mod stream;

pub use stream::{
    Stream, make_stream, stream_direction, stream_element_type, stream_external_format,
    stream_implementation, stream_slot, stream_state,
};
