mod access;
mod construction;
mod metadata;
mod mutable;

#[allow(clippy::wildcard_imports)]
pub use metadata::*;

pub use access::{aref, array_in_bounds_p, array_row_major_index, bit, row_major_aref, svref};
pub use construction::{make_array, vector};
pub use mutable::{
    adjust_array, array_has_fill_pointer_p, fill_pointer, vector_pop, vector_push,
    vector_push_extend,
};
