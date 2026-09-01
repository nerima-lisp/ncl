use ncl_syntax::Form;

pub(super) fn bind_hash_value_and_key(
    form: &Form,
    value_binding: &Form,
    key_binding: &Form,
    key: &Form,
    table: &Form,
    body: Form,
) -> Form {
    Form::list(
        vec![
            Form::atom("LET", form.span),
            Form::list(
                vec![
                    Form::list(
                        vec![
                            value_binding.clone(),
                            Form::list(
                                vec![
                                    Form::atom("GETHASH", form.span),
                                    key.clone(),
                                    table.clone(),
                                ],
                                form.span,
                            ),
                        ],
                        form.span,
                    ),
                    Form::list(vec![key_binding.clone(), key.clone()], form.span),
                ],
                form.span,
            ),
            body,
        ],
        form.span,
    )
}
