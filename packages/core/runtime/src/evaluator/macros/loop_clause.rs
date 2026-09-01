use ncl_syntax::Form;

use crate::{environment::names_equal, evaluator::helpers::atom_name};

pub(super) fn across_clause(items: &[Form]) -> bool {
    items
        .get(3)
        .and_then(atom_name)
        .is_some_and(|name| names_equal(name, "ACROSS"))
}
