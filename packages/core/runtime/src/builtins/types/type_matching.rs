mod array_specs;
mod cons_vector_specs;
mod dispatch;
pub(super) use dispatch::{resolve_type_designator_in, type_matches_designator_in};
mod numeric_specs;
mod spec_utils;
mod type_name_table;

pub(super) use dispatch::type_matches_designator;
pub(super) use numeric_specs::{byte_type_size, integer_type_bound};
pub(super) use spec_utils::{
    invalid_type_spec, is_type_wildcard, require_type_spec_arity, type_spec_size,
};

#[cfg(test)]
mod builtin_types_tests;
