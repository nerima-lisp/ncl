use ncl_syntax::Form;

pub(super) fn count_step(form: &Form, value: Form, name: Form) -> Form {
    Form::list(
        vec![
            Form::atom("WHEN", form.span),
            value,
            Form::list(vec![Form::atom("INCF", form.span), name], form.span),
        ],
        form.span,
    )
}

pub(super) fn sum_step(form: &Form, value: Form, name: Form) -> Form {
    Form::list(
        vec![Form::atom("INCF", form.span), name, value],
        form.span,
    )
}

pub(super) fn append_step(form: &Form, value: Form, name: Form) -> Form {
    let item = Form::atom(format!("NCL-LOOP-APPEND-{}", form.span.start), form.span);
    Form::list(
        vec![
            Form::atom("DOLIST", form.span),
            Form::list(vec![item.clone(), value], form.span),
            Form::list(
                vec![Form::atom("PUSH", form.span), item, name],
                form.span,
            ),
        ],
        form.span,
    )
}
