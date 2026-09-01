use ncl_syntax::Form;

use crate::{environment::names_equal, evaluator::helpers::atom_name, Runtime, RuntimeError};

pub(super) struct LoopRepeatExpansion {
    pub body: Vec<Form>,
    pub repeat_count: Form,
    pub collect_form: Option<Form>,
    pub finally_form: Option<Form>,
}

pub(super) fn expand_loop_repeat(
    form: &Form,
    items: &[Form],
    count_name: &Form,
    collect_name: &Form,
) -> Result<LoopRepeatExpansion, RuntimeError> {
    if items.len() < 3 {
        return Err(Runtime::invalid(
            "LOOP REPEAT clause requires a count",
            form.span,
        ));
    }
    let mut body_start = usize::from(
        items
            .get(3)
            .and_then(atom_name)
            .is_some_and(|name| names_equal(name, "DO")),
    ) + 3;
    let mut collect_form = None;
    if items
        .get(body_start)
        .and_then(atom_name)
        .is_some_and(|name| names_equal(name, "COLLECT"))
    {
        if items.len() <= body_start + 1 {
            return Err(Runtime::invalid(
                "LOOP COLLECT clause requires a form",
                form.span,
            ));
        }
        collect_form = Some(items[body_start + 1].clone());
        body_start += 2;
    }
    let exhausted = Form::list(
        vec![
            Form::atom("WHEN", form.span),
            Form::list(
                vec![
                    Form::atom("<=", form.span),
                    count_name.clone(),
                    Form::atom("0", form.span),
                ],
                form.span,
            ),
            Form::list(vec![Form::atom("RETURN", form.span)], form.span),
        ],
        form.span,
    );
    let mut body = vec![exhausted];
    if let Some(value) = collect_form.clone() {
        body.push(Form::list(
            vec![Form::atom("PUSH", form.span), value, collect_name.clone()],
            form.span,
        ));
    }
    body.extend(items[body_start..].iter().cloned());
    let mut finally_form = None;
    if let Some(finally_offset) = body
        .iter()
        .position(|item| atom_name(item).is_some_and(|name| names_equal(name, "FINALLY")))
    {
        let finally_items = body.split_off(finally_offset + 1);
        body.pop();
        if finally_items.is_empty() {
            return Err(Runtime::invalid(
                "LOOP FINALLY clause requires a form",
                form.span,
            ));
        }
        finally_form = Some(if finally_items.len() == 1 {
            finally_items[0].clone()
        } else {
            Form::list(
                std::iter::once(Form::atom("PROGN", form.span))
                    .chain(finally_items)
                    .collect(),
                form.span,
            )
        });
    }
    body.push(Form::list(
        vec![Form::atom("DECF", form.span), count_name.clone()],
        form.span,
    ));
    Ok(LoopRepeatExpansion {
        body,
        repeat_count: items[2].clone(),
        collect_form,
        finally_form,
    })
}
