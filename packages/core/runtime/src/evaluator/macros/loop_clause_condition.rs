use ncl_syntax::Form;

use crate::{environment::names_equal, evaluator::helpers::atom_name, Runtime, RuntimeError};

pub(super) fn parse_loop_clause_condition(
    form: &Form,
    items: &[Form],
    body_start: &mut usize,
) -> Result<Option<Form>, RuntimeError> {
    let Some(clause_name) = items.get(*body_start).and_then(atom_name) else {
        return Ok(None);
    };
    if !names_equal(clause_name, "WHEN") && !names_equal(clause_name, "UNLESS") {
        return Ok(None);
    }
    if items.len() <= *body_start + 1 {
        return Err(Runtime::invalid(
            "LOOP WHEN/UNLESS clause requires a test",
            form.span,
        ));
    }
    let test = items[*body_start + 1].clone();
    *body_start += 2;
    Ok(Some(if names_equal(clause_name, "WHEN") {
        test
    } else {
        Form::list(vec![Form::atom("NOT", form.span), test], form.span)
    }))
}
