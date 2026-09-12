use ncl_syntax::Form;

use crate::{Runtime, RuntimeError};

pub(super) fn expand_loop_collect(
    form: &Form,
    items: &[Form],
    collect_name: &Form,
) -> Result<(Vec<Form>, Form), RuntimeError> {
    if items.len() < 3 {
        return Err(Runtime::invalid(
            "LOOP COLLECT clause requires a form",
            form.span,
        ));
    }
    let body = std::iter::once(Form::list(
        vec![
            Form::atom("PUSH", form.span),
            items[2].clone(),
            collect_name.clone(),
        ],
        form.span,
    ))
    .chain(items[3..].iter().cloned())
    .collect();
    Ok((body, items[2].clone()))
}
