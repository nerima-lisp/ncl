use ncl_syntax::Form;

pub(super) fn finalize(
    form: &Form,
    named_block: Option<Form>,
    body: Vec<Form>,
    repeat_count: Option<Form>,
    collect_form: Option<Form>,
    finally_form: Option<Form>,
    count_name: Form,
    collect_name: Form,
) -> Form {
    let tag = Form::atom(format!("#:NCL-LOOP-{}", form.span.start), form.span);
    let mut tagbody = vec![Form::atom("TAGBODY", form.span), tag.clone()];
    tagbody.extend(body);
    tagbody.push(Form::list(
        vec![Form::atom("GO", form.span), tag],
        form.span,
    ));
    let block = Form::list(
        vec![
            Form::atom("BLOCK", form.span),
            Form::atom("NIL", form.span),
            Form::list(tagbody, form.span),
        ],
        form.span,
    );
    let block = named_block.map_or(block.clone(), |name| {
        Form::list(vec![Form::atom("BLOCK", form.span), name, block], form.span)
    });
    if repeat_count.is_some() || collect_form.is_some() {
        let mut bindings = Vec::new();
        if let Some(count) = repeat_count {
            bindings.push(Form::list(vec![count_name, count], form.span));
        }
        if collect_form.is_some() {
            bindings.push(Form::list(
                vec![collect_name.clone(), Form::atom("NIL", form.span)],
                form.span,
            ));
        }
        let result = if collect_form.is_some() {
            Form::list(
                vec![Form::atom("NREVERSE", form.span), collect_name],
                form.span,
            )
        } else {
            block.clone()
        };
        let block_result = if collect_form.is_some() {
            Form::list(
                vec![Form::atom("PROGN", form.span), block, result],
                form.span,
            )
        } else {
            result
        };
        let block_result = finally_form.map_or(block_result.clone(), |finally| {
            Form::list(
                vec![Form::atom("PROGN", form.span), block_result, finally],
                form.span,
            )
        });
        Form::list(
            vec![
                Form::atom("LET", form.span),
                Form::list(bindings, form.span),
                block_result,
            ],
            form.span,
        )
    } else {
        block
    }
}
