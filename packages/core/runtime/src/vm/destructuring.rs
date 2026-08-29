mod lambda_list_parameters;
mod pattern;
mod specification;

pub(super) use pattern::{destructure_dotted_parts, destructure_value};
pub(super) use specification::destructure_specification;
