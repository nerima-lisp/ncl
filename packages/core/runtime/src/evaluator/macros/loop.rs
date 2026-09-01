use ncl_syntax::{Form, FormKind};

use crate::{environment::names_equal, evaluator::helpers::atom_name, Runtime, RuntimeError};

impl Runtime {
    pub(super) fn expand_builtin_loop(form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        let mut body = items[1..].to_vec();
        let mut repeat_count = None;
        let mut collect_form = None;
        let count_name = Form::atom(format!("NCL-LOOP-COUNT-{}", form.span.start), form.span);
        let collect_name = Form::atom(format!("NCL-LOOP-COLLECT-{}", form.span.start), form.span);
        if let Some(clause) = items.get(1).and_then(atom_name) {
            if names_equal(clause, "WHILE") || names_equal(clause, "UNTIL") {
                if items.len() < 3 {
                    return Err(Self::invalid(
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
                body = vec![Form::list(
                    vec![
                        Form::atom(guard_operator, form.span),
                        items[2].clone(),
                        return_form,
                    ],
                    form.span,
                )];
                body.extend(items[body_start..].iter().cloned());
            } else if names_equal(clause, "REPEAT") {
                if items.len() < 3 {
                    return Err(Self::invalid(
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
                if items
                    .get(body_start)
                    .and_then(atom_name)
                    .is_some_and(|name| names_equal(name, "COLLECT"))
                {
                    if items.len() <= body_start + 1 {
                        return Err(Self::invalid(
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
                body = vec![exhausted];
                if let Some(value) = collect_form.clone() {
                    body.push(Form::list(
                        vec![Form::atom("PUSH", form.span), value, collect_name.clone()],
                        form.span,
                    ));
                }
                body.extend(items[body_start..].iter().cloned());
                body.push(Form::list(
                    vec![Form::atom("DECF", form.span), count_name.clone()],
                    form.span,
                ));
                repeat_count = Some(items[2].clone());
            } else if names_equal(clause, "COLLECT") {
                if items.len() < 3 {
                    return Err(Self::invalid(
                        "LOOP COLLECT clause requires a form",
                        form.span,
                    ));
                }
                collect_form = Some(items[2].clone());
                body = vec![Form::list(
                    vec![
                        Form::atom("PUSH", form.span),
                        items[2].clone(),
                        collect_name.clone(),
                    ],
                    form.span,
                )];
                body.extend(items[3..].iter().cloned());
            }
        }

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
            Ok(Form::list(
                vec![
                    Form::atom("LET", form.span),
                    Form::list(bindings, form.span),
                    block_result,
                ],
                form.span,
            ))
        } else {
            Ok(block)
        }
    }
}
