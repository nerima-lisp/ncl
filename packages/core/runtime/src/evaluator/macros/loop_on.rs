use ncl_syntax::Form;

use crate::{environment::names_equal, evaluator::helpers::atom_name, Runtime, RuntimeError};

pub(super) fn expand_loop_for_on(
    form: &Form,
    items: &[Form],
) -> Result<Option<Form>, RuntimeError> {
    if !items
        .get(3)
        .and_then(atom_name)
        .is_some_and(|name| names_equal(name, "ON"))
    {
        return Ok(None);
    }
    if items.len() < 5 {
        return Err(Runtime::invalid(
            "LOOP FOR ON requires a variable and list form",
            form.span,
        ));
    }
    let variable = items[2].clone();
    let (step, body_start) = if items
        .get(5)
        .and_then(atom_name)
        .is_some_and(|name| names_equal(name, "BY"))
    {
        if items.len() < 7 {
            return Err(Runtime::invalid(
                "LOOP FOR ON BY requires a function form",
                form.span,
            ));
        }
        (
            Form::list(vec![items[6].clone(), variable.clone()], form.span),
            7,
        )
    } else {
        (
            Form::list(
                vec![Form::atom("CDR", form.span), variable.clone()],
                form.span,
            ),
            5,
        )
    };
    let mut loop_items = vec![
        Form::atom("LOOP", form.span),
        Form::atom("FOR", form.span),
        variable.clone(),
        Form::atom("=", form.span),
        items[4].clone(),
        Form::atom("THEN", form.span),
        step,
        Form::atom("WHILE", form.span),
        variable,
    ];
    loop_items.extend(items[body_start..].iter().cloned());
    Ok(Some(Form::list(loop_items, form.span)))
}
