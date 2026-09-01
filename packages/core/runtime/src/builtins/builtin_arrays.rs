mod access;
mod construction;
mod metadata;

#[allow(clippy::wildcard_imports)]
pub use metadata::*;

pub use access::{aref, array_in_bounds_p, array_row_major_index, bit, row_major_aref, svref};
pub use construction::{adjust_array, fill_pointer, make_array, vector, vector_push, vector_push_extend};
