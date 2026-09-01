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
            } else if names_equal(clause, "WITH") {
                let mut bindings = Vec::new();
                let mut body_start = 2;
                loop {
                    if items.len() <= body_start + 2
                        || items.get(body_start + 1).and_then(atom_name) != Some("=")
                    {
                        return Err(Self::invalid(
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
                let mut let_items = vec![
                    Form::atom("LET", form.span),
                    Form::list(bindings, form.span),
                ];
                let_items.extend(items[body_start..].iter().cloned());
                return Ok(Form::list(let_items, form.span));
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
            } else if names_equal(clause, "FOR") {
                if items
                    .get(3)
                    .and_then(atom_name)
                    .is_some_and(|name| names_equal(name, "IN"))
                {
                    if items.len() < 5 {
                        return Err(Self::invalid(
                            "LOOP FOR IN requires a variable and list form",
                            form.span,
                        ));
                    }
                    let variable = items[2].clone();
                    let mut body_start = 5;
                    if items
                        .get(body_start)
                        .and_then(atom_name)
                        .is_some_and(|name| names_equal(name, "DO"))
                    {
                        body_start += 1;
                    }
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
                    let mut dolist_items = vec![
                        Form::atom("DOLIST", form.span),
                        Form::list(vec![variable, items[4].clone()], form.span),
                    ];
                    if let Some(value) = collect_form.clone() {
                        dolist_items.push(Form::list(
                            vec![Form::atom("PUSH", form.span), value, collect_name.clone()],
                            form.span,
                        ));
                    }
                    dolist_items.extend(items[body_start..].iter().cloned());
                    let dolist = Form::list(dolist_items, form.span);
                    if collect_form.is_some() {
                        return Ok(Form::list(
                            vec![
                                Form::atom("LET", form.span),
                                Form::list(
                                    vec![Form::list(
                                        vec![collect_name.clone(), Form::atom("NIL", form.span)],
                                        form.span,
                                    )],
                                    form.span,
                                ),
                                Form::list(
                                    vec![
                                        Form::atom("PROGN", form.span),
                                        dolist,
                                        Form::list(
                                            vec![Form::atom("NREVERSE", form.span), collect_name],
                                            form.span,
                                        ),
                                    ],
                                    form.span,
                                ),
                            ],
                            form.span,
                        ));
                    }
                    return Ok(dolist);
                }
                let limit_clause = items.get(5).and_then(atom_name);
                let descending = limit_clause
                    .is_some_and(|name| names_equal(name, "DOWNTO") || names_equal(name, "ABOVE"));
                let inclusive = limit_clause
                    .is_some_and(|name| names_equal(name, "TO") || names_equal(name, "DOWNTO"));
                if items.len() < 7
                    || !items
                        .get(3)
                        .and_then(atom_name)
                        .is_some_and(|name| names_equal(name, "FROM"))
                    || !limit_clause.is_some_and(|name| {
                        names_equal(name, "TO")
                            || names_equal(name, "BELOW")
                            || names_equal(name, "ABOVE")
                            || names_equal(name, "DOWNTO")
                    })
                {
                    return Err(Self::invalid(
                        "LOOP FOR requires a variable, FROM form, and TO form",
                        form.span,
                    ));
                }
                let variable = items[2].clone();
                let mut sum_form = None;
                let mut sum_name = None;
                let mut extremum_form = None;
                let mut extremum_name = None;
                let mut maximize = false;
                let mut body_start = 7;
                let step_form = if items
                    .get(body_start)
                    .and_then(atom_name)
                    .is_some_and(|name| names_equal(name, "BY"))
                {
                    if items.len() <= body_start + 1 {
                        return Err(Self::invalid(
                            "LOOP BY clause requires a step form",
                            form.span,
                        ));
                    }
                    let step = items[body_start + 1].clone();
                    body_start += 2;
                    step
                } else {
                    Form::atom("1", form.span)
                };
                let step_operator = if descending { "-" } else { "+" };
                let step = Form::list(
                    vec![
                        Form::atom(step_operator, form.span),
                        variable.clone(),
                        step_form,
                    ],
                    form.span,
                );
                let binding = Form::list(vec![variable.clone(), items[4].clone(), step], form.span);
                if items
                    .get(body_start)
                    .and_then(atom_name)
                    .is_some_and(|name| names_equal(name, "DO"))
                {
                    body_start += 1;
                }
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
                if items
                    .get(body_start)
                    .and_then(atom_name)
                    .is_some_and(|name| {
                        names_equal(name, "SUM")
                            || names_equal(name, "MAXIMIZE")
                            || names_equal(name, "MINIMIZE")
                    })
                {
                    if items.len() <= body_start + 1 {
                        return Err(Self::invalid("LOOP SUM clause requires a form", form.span));
                    }
                    let aggregate_clause = atom_name(&items[body_start]).unwrap();
                    let is_sum = names_equal(aggregate_clause, "SUM");
                    if is_sum {
                        sum_form = Some(items[body_start + 1].clone());
                    } else {
                        maximize = names_equal(aggregate_clause, "MAXIMIZE");
                        extremum_form = Some(items[body_start + 1].clone());
                    }
                    let aggregate_name = if items
                        .get(body_start + 2)
                        .and_then(atom_name)
                        .is_some_and(|name| names_equal(name, "INTO"))
                    {
                        if items.len() <= body_start + 3 {
                            return Err(Self::invalid(
                                "LOOP aggregate INTO requires a variable",
                                form.span,
                            ));
                        }
                        body_start += 4;
                        items[body_start - 1].clone()
                    } else {
                        body_start += 2;
                        Form::atom(format!("NCL-LOOP-AGG-{}", form.span.start), form.span)
                    };
                    if is_sum {
                        sum_name = Some(aggregate_name);
                    } else {
                        extremum_name = Some(aggregate_name);
                    }
                }
                let termination_operator = match (descending, inclusive) {
                    (false, true) => ">",
                    (false, false) => ">=",
                    (true, true) => "<",
                    (true, false) => "<=",
                };
                let termination = vec![Form::list(
                    vec![
                        Form::atom(termination_operator, form.span),
                        variable,
                        items[6].clone(),
                    ],
                    form.span,
                )];
                let mut do_items = vec![
                    Form::atom("DO", form.span),
                    Form::list(vec![binding], form.span),
                ];
                let termination = if collect_form.is_some() {
                    Form::list(
                        vec![
                            termination[0].clone(),
                            Form::list(
                                vec![Form::atom("NREVERSE", form.span), collect_name.clone()],
                                form.span,
                            ),
                        ],
                        form.span,
                    )
                } else {
                    Form::list(termination, form.span)
                };
                do_items.push(termination);
                if let Some(value) = collect_form.clone() {
                    do_items.push(Form::list(
                        vec![Form::atom("PUSH", form.span), value, collect_name.clone()],
                        form.span,
                    ));
                }
                if let (Some(value), Some(name)) = (sum_form, sum_name.clone()) {
                    do_items.push(Form::list(
                        vec![Form::atom("INCF", form.span), name, value],
                        form.span,
                    ));
                }
                if let (Some(value), Some(name)) = (extremum_form, extremum_name.clone()) {
                    let comparison = if maximize { ">" } else { "<" };
                    do_items.push(Form::list(
                        vec![
                            Form::atom("WHEN", form.span),
                            Form::list(
                                vec![
                                    Form::atom("OR", form.span),
                                    Form::list(
                                        vec![Form::atom("NULL", form.span), name.clone()],
                                        form.span,
                                    ),
                                    Form::list(
                                        vec![
                                            Form::atom(comparison, form.span),
                                            value.clone(),
                                            name.clone(),
                                        ],
                                        form.span,
                                    ),
                                ],
                                form.span,
                            ),
                            Form::list(vec![Form::atom("SETQ", form.span), name, value], form.span),
                        ],
                        form.span,
                    ));
                }
                do_items.extend(items[body_start..].iter().cloned());
                let do_form = Form::list(do_items, form.span);
                if collect_form.is_some() || sum_name.is_some() || extremum_name.is_some() {
                    let mut bindings = vec![];
                    if collect_form.is_some() {
                        bindings.push(Form::list(
                            vec![collect_name.clone(), Form::atom("NIL", form.span)],
                            form.span,
                        ));
                    }
                    if let Some(name) = sum_name.clone() {
                        bindings.push(Form::list(
                            vec![name.clone(), Form::atom("0", form.span)],
                            form.span,
                        ));
                    }
                    if let Some(name) = extremum_name.clone() {
                        bindings.push(Form::list(
                            vec![name, Form::atom("NIL", form.span)],
                            form.span,
                        ));
                    }
                    let result = if collect_form.is_some() {
                        Form::list(
                            vec![Form::atom("NREVERSE", form.span), collect_name],
                            form.span,
                        )
                    } else if let Some(name) = sum_name {
                        name
                    } else {
                        extremum_name.expect("aggregate name is present")
                    };
                    let body_result = Form::list(
                        vec![Form::atom("PROGN", form.span), do_form, result],
                        form.span,
                    );
                    return Ok(Form::list(
                        vec![
                            Form::atom("LET", form.span),
                            Form::list(bindings, form.span),
                            body_result,
                        ],
                        form.span,
                    ));
                }
                return Ok(do_form);
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
