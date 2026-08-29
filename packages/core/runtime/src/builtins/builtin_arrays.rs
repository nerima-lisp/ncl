mod access;
mod construction;
mod metadata;

#[allow(clippy::wildcard_imports)]
pub(super) use metadata::*;

pub(super) use access::{
    aref, array_in_bounds_p, array_row_major_index, bit, row_major_aref, svref,
};
pub(super) use construction::{make_array, vector};
