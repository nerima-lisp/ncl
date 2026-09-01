use ncl_syntax::Form;

use crate::{environment::names_equal, evaluator::helpers::atom_name, Runtime, RuntimeError};

pub(super) fn expand_loop_with(form: &Form, items: &[Form]) -> Result<Form, RuntimeError> {
    let mut bindings = Vec::new();
    let mut body_start = 2;
    loop {
        if items.len() <= body_start + 2
            || items.get(body_start + 1).and_then(atom_name) != Some("=")
        {
            return Err(Runtime::invalid(
                "LOOP WITH requires a variable, =, and an initial value",
                form.span,
            ));
        }
        bindings.push(Form::list(
            vec![items[body_start].clone(), items[body_start + 2].clone()],
            form.span,
        ));
        body_start += 3;
        if items
            .get(body_start)
            .and_then(atom_name)
            .is_some_and(|name| names_equal(name, "AND"))
        {
            body_start += 1;
            continue;
        }
        break;
    }
    if items
        .get(body_start)
        .and_then(atom_name)
        .is_some_and(|name| names_equal(name, "DO"))
    {
        body_start += 1;
    }
    let mut let_items = vec![Form::atom("LET", form.span), Form::list(bindings, form.span)];
    let_items.extend(items[body_start..].iter().cloned());
    Ok(Form::list(let_items, form.span))
}
