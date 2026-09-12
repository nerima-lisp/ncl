use ncl_syntax::Form;

use crate::{Runtime, RuntimeError};

use super::loop_control::clause_offset;

pub(super) fn expand_loop_entry_clause(
    form: &Form,
    items: &[Form],
) -> Result<Option<Form>, RuntimeError> {
    if let Some(offset) = clause_offset(items, "RETURN") {
        return Runtime::expand_loop_return_clause(form, items, offset).map(Some);
    }
    if let Some(offset) = clause_offset(items, "INITIALLY") {
        return Runtime::expand_loop_initially_clause(form, items, offset).map(Some);
    }
    if let Some(offset) = clause_offset(items, "FINALLY") {
        return Runtime::expand_loop_finally_clause(form, items, offset).map(Some);
    }
    Ok(None)
}
