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
