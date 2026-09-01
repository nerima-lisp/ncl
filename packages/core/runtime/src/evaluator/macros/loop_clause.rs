use ncl_syntax::Form;

use crate::{
    environment::names_equal,
    evaluator::helpers::atom_name,
    Runtime,
    RuntimeError,
};

pub(super) struct AcrossClause {
    pub(super) variable: Form,
    pub(super) vector: Form,
    pub(super) body_start: usize,
}

pub(super) fn parse_across_clause(
    form: &Form,
    items: &[Form],
) -> Result<Option<AcrossClause>, RuntimeError> {
    if !items
        .get(3)
        .and_then(atom_name)
        .is_some_and(|name| names_equal(name, "ACROSS"))
    {
        return Ok(None);
    }
    if items.len() < 5 {
        return Err(Runtime::invalid(
            "LOOP FOR ACROSS requires a variable and vector form",
            form.span,
        ));
    }
    Ok(Some(AcrossClause {
        variable: items[2].clone(),
        vector: items[4].clone(),
        body_start: 5,
    }))
}
