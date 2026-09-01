use ncl_syntax::Form;

use crate::{evaluator::helpers::atom_name, Runtime, RuntimeError};

use super::for_in::expand_loop_for_in;
use super::loop_hash::expand_loop_hash_being;
use super::loop_on::expand_loop_for_on;

pub(super) fn expand_loop_for_prefix(
    form: &Form,
    items: &[Form],
) -> Result<Option<Form>, RuntimeError> {
    if items
        .get(3)
        .and_then(atom_name)
        .is_some_and(|name| crate::environment::names_equal(name, "BEING"))
    {
        return Ok(Some(expand_loop_hash_being(form, items)?.ok_or_else(
            || Runtime::invalid("LOOP hash-table expansion failed", form.span),
        )?));
    }
    if let Some(expanded) = expand_loop_for_in(form, items)? {
        return Ok(Some(expanded));
    }
    if let Some(expanded) = expand_loop_for_on(form, items)? {
        return Ok(Some(expanded));
    }
    Ok(None)
}
