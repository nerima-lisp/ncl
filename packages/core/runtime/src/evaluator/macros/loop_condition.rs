use ncl_syntax::Form;

use crate::{environment::names_equal, evaluator::helpers::atom_name, Runtime, RuntimeError};

pub(super) fn expand_loop_condition(
    form: &Form,
    items: &[Form],
) -> Result<Option<Vec<Form>>, RuntimeError> {
    let Some(clause) = items.get(1).and_then(atom_name) else {
        return Ok(None);
    };
    if !names_equal(clause, "WHILE") && !names_equal(clause, "UNTIL") {
        return Ok(None);
    }
    if items.len() < 3 {
        return Err(Runtime::invalid(
            "LOOP condition clause requires a test",
            form.span,
        ));
    }
    let stop_on_true = names_equal(clause, "UNTIL");
    let body_start = usize::from(
        items
            .get(3)
            .and_then(atom_name)
            .is_some_and(|name| names_equal(name, "DO")),
    ) + 3;
    let return_form = Form::list(vec![Form::atom("RETURN", form.span)], form.span);
    let guard_operator = if stop_on_true { "WHEN" } else { "UNLESS" };
    let mut body = vec![Form::list(
        vec![
            Form::atom(guard_operator, form.span),
            items[2].clone(),
            return_form,
        ],
        form.span,
    )];
    body.extend(items[body_start..].iter().cloned());
    Ok(Some(body))
}
