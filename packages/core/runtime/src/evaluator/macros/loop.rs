use ncl_syntax::{Form, FormKind};

use crate::{environment::names_equal, evaluator::helpers::atom_name, Runtime, RuntimeError};

use super::loop_aggregate::{append_step, count_step, sum_step};

impl Runtime {
    pub(super) fn expand_builtin_loop(form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        let mut body = items[1..].to_vec();
        let mut repeat_count = None;
        let mut collect_form = None;
        let count_name = Form::atom(format!("NCL-LOOP-COUNT-{}", form.span.start), form.span);
        let mut collect_name =
            Form::atom(format!("NCL-LOOP-COLLECT-{}", form.span.start), form.span);
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
                    let mut sum_form = None;
                    let mut sum_name = None;
                    let mut count_form = None;
                    let mut count_result_name = None;
                    let mut extremum_form = None;
                    let mut extremum_name = None;
                    let mut maximize = false;
                    let mut append = false;
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
                        .is_some_and(|name| {
                            names_equal(name, "COLLECT") || names_equal(name, "APPEND")
                        })
                    {
                        if items.len() <= body_start + 1 {
                            return Err(Self::invalid(
                                "LOOP COLLECT clause requires a form",
                                form.span,
                            ));
                        }
                        collect_form = Some(items[body_start + 1].clone());
                        append = names_equal(atom_name(&items[body_start]).unwrap(), "APPEND");
                        body_start += 2;
                        if items
                            .get(body_start)
                            .and_then(atom_name)
                            .is_some_and(|name| names_equal(name, "INTO"))
                        {
                            if items.len() <= body_start + 1 {
                                return Err(Self::invalid(
                                    "LOOP APPEND/COLLECT INTO requires a variable",
                                    form.span,
                                ));
                            }
                            collect_name = items[body_start + 1].clone();
                            body_start += 2;
                        }
                    }
                    if items
                        .get(body_start)
                        .and_then(atom_name)
                        .is_some_and(|name| {
                            names_equal(name, "SUM")
                                || names_equal(name, "COUNT")
                                || names_equal(name, "MAXIMIZE")
                                || names_equal(name, "MINIMIZE")
                        })
                    {
                        if items.len() <= body_start + 1 {
                            return Err(Self::invalid(
                                "LOOP aggregate clause requires a form",
                                form.span,
                            ));
                        }
                        let aggregate_clause = atom_name(&items[body_start]).unwrap();
                        let is_sum = names_equal(aggregate_clause, "SUM");
                        if is_sum {
                            sum_form = Some(items[body_start + 1].clone());
                        } else if names_equal(aggregate_clause, "COUNT") {
                            count_form = Some(items[body_start + 1].clone());
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
                        } else if count_form.is_some() {
                            count_result_name = Some(aggregate_name);
                        } else {
                            extremum_name = Some(aggregate_name);
                        }
                    }
                    let mut dolist_items = vec![
                        Form::atom("DOLIST", form.span),
                        Form::list(vec![variable, items[4].clone()], form.span),
                    ];
                    if let Some(value) = collect_form.clone() {
                        if append {
                            dolist_items.push(append_step(form, value, collect_name.clone()));
                        } else {
                            dolist_items.push(Form::list(
                                vec![Form::atom("PUSH", form.span), value, collect_name.clone()],
                                form.span,
                            ));
                        }
                    }
                    if let (Some(value), Some(name)) = (sum_form, sum_name.clone()) {
                        dolist_items.push(sum_step(form, value, name));
                    }
                    if let (Some(value), Some(name)) = (count_form, count_result_name.clone()) {
                        dolist_items.push(count_step(form, value, name));
                    }
                    if let (Some(value), Some(name)) = (extremum_form, extremum_name.clone()) {
                        let comparison = if maximize { ">" } else { "<" };
                        let candidate = Form::atom(
                            format!("NCL-LOOP-CANDIDATE-{}", form.span.start),
                            form.span,
                        );
                        let update = Form::list(
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
                                                candidate.clone(),
                                                name.clone(),
                                            ],
                                            form.span,
                                        ),
                                    ],
                                    form.span,
                                ),
                                Form::list(
                                    vec![Form::atom("SETQ", form.span), name, candidate.clone()],
                                    form.span,
                                ),
                            ],
                            form.span,
                        );
                        dolist_items.push(Form::list(
                            vec![
                                Form::atom("LET", form.span),
                                Form::list(
                                    vec![Form::list(vec![candidate, value], form.span)],
                                    form.span,
                                ),
                                update,
                            ],
                            form.span,
                        ));
                    }
                    dolist_items.extend(items[body_start..].iter().cloned());
                    let dolist = Form::list(dolist_items, form.span);
                    if collect_form.is_some()
                        || sum_name.is_some()
                        || count_result_name.is_some()
                        || extremum_name.is_some()
                    {
                        let mut bindings = vec![];
                        if collect_form.is_some() {
                            bindings.push(Form::list(
                                vec![collect_name.clone(), Form::atom("NIL", form.span)],
                                form.span,
                            ));
                        }
                        if let Some(name) = sum_name.clone() {
                            bindings.push(Form::list(
                                vec![name, Form::atom("0", form.span)],
                                form.span,
                            ));
                        }
                        if let Some(name) = count_result_name.clone() {
                            bindings.push(Form::list(
                                vec![name, Form::atom("0", form.span)],
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
                        } else if let Some(name) = count_result_name {
                            name
                        } else {
                            extremum_name.expect("aggregate name is present")
                        };
                        let body_result = Form::list(
                            vec![Form::atom("PROGN", form.span), dolist, result],
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
                    return Ok(dolist);
                }
                if items
                    .get(3)
                    .and_then(atom_name)
                    .is_some_and(|name| names_equal(name, "ON"))
                {
                    if items.len() < 5 {
                        return Err(Self::invalid(
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
                            return Err(Self::invalid(
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
                    return Ok(Form::list(loop_items, form.span));
                }
                if items
                    .get(3)
                    .and_then(atom_name)
                    .is_some_and(|name| names_equal(name, "ACROSS"))
                {
                    if items.len() < 5 {
                        return Err(Self::invalid(
                            "LOOP FOR ACROSS requires a variable and vector form",
                            form.span,
                        ));
                    }
                    let variable = items[2].clone();
                    let index =
                        Form::atom(format!("NCL-LOOP-INDEX-{}", form.span.start), form.span);
                    let vector =
                        Form::atom(format!("NCL-LOOP-VECTOR-{}", form.span.start), form.span);
                    let mut body_start = 5;
                    let mut sum_form = None;
                    let mut sum_name = None;
                    let mut count_form = None;
                    let mut count_name = None;
                    let mut extremum_form = None;
                    let mut extremum_name = None;
                    let mut maximize = false;
                    if items
                        .get(body_start)
                        .and_then(atom_name)
                        .is_some_and(|name| names_equal(name, "DO"))
                    {
                        body_start += 1;
                    }
                    let mut loop_body = Vec::new();
                    if items
                        .get(body_start)
                        .and_then(atom_name)
                        .is_some_and(|name| {
                            names_equal(name, "COLLECT") || names_equal(name, "APPEND")
                        })
                    {
                        if items.len() <= body_start + 1 {
                            return Err(Self::invalid(
                                "LOOP COLLECT clause requires a form",
                                form.span,
                            ));
                        }
                        collect_form = Some(items[body_start + 1].clone());
                        let append = names_equal(atom_name(&items[body_start]).unwrap(), "APPEND");
                        if append {
                            loop_body.push(append_step(
                                form,
                                items[body_start + 1].clone(),
                                collect_name.clone(),
                            ));
                        } else {
                            loop_body.push(Form::list(
                                vec![
                                    Form::atom("PUSH", form.span),
                                    items[body_start + 1].clone(),
                                    collect_name.clone(),
                                ],
                                form.span,
                            ));
                        }
                        body_start += 2;
                        if items
                            .get(body_start)
                            .and_then(atom_name)
                            .is_some_and(|name| names_equal(name, "INTO"))
                        {
                            if items.len() <= body_start + 1 {
                                return Err(Self::invalid(
                                    "LOOP APPEND/COLLECT INTO requires a variable",
                                    form.span,
                                ));
                            }
                            collect_name = items[body_start + 1].clone();
                            body_start += 2;
                        }
                    }
                    if items
                        .get(body_start)
                        .and_then(atom_name)
                        .is_some_and(|name| {
                            names_equal(name, "SUM")
                                || names_equal(name, "COUNT")
                                || names_equal(name, "MAXIMIZE")
                                || names_equal(name, "MINIMIZE")
                        })
                    {
                        if items.len() <= body_start + 1 {
                            return Err(Self::invalid(
                                "LOOP aggregate clause requires a form",
                                form.span,
                            ));
                        }
                        let aggregate_clause = atom_name(&items[body_start]).unwrap();
                        let is_sum = names_equal(aggregate_clause, "SUM");
                        let aggregate_form = items[body_start + 1].clone();
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
                            sum_form = Some(aggregate_form);
                            sum_name = Some(aggregate_name);
                        } else if names_equal(aggregate_clause, "COUNT") {
                            count_form = Some(aggregate_form);
                            count_name = Some(aggregate_name);
                        } else {
                            maximize = names_equal(aggregate_clause, "MAXIMIZE");
                            extremum_form = Some(aggregate_form);
                            extremum_name = Some(aggregate_name);
                        }
                    }
                    if let (Some(value), Some(name)) = (sum_form.clone(), sum_name.clone()) {
                        loop_body.push(sum_step(form, value, name));
                    }
                    if let (Some(value), Some(name)) = (count_form.clone(), count_name.clone()) {
                        loop_body.push(count_step(form, value, name));
                    }
                    if let (Some(value), Some(name)) =
                        (extremum_form.clone(), extremum_name.clone())
                    {
                        let comparison = if maximize { ">" } else { "<" };
                        let candidate = Form::atom(
                            format!("NCL-LOOP-CANDIDATE-{}", form.span.start),
                            form.span,
                        );
                        loop_body.push(Form::list(
                            vec![
                                Form::atom("LET", form.span),
                                Form::list(
                                    vec![Form::list(vec![candidate.clone(), value], form.span)],
                                    form.span,
                                ),
                                Form::list(
                                    vec![
                                        Form::atom("WHEN", form.span),
                                        Form::list(
                                            vec![
                                                Form::atom("OR", form.span),
                                                Form::list(
                                                    vec![
                                                        Form::atom("NULL", form.span),
                                                        name.clone(),
                                                    ],
                                                    form.span,
                                                ),
                                                Form::list(
                                                    vec![
                                                        Form::atom(comparison, form.span),
                                                        candidate.clone(),
                                                        name.clone(),
                                                    ],
                                                    form.span,
                                                ),
                                            ],
                                            form.span,
                                        ),
                                        Form::list(
                                            vec![Form::atom("SETQ", form.span), name, candidate],
                                            form.span,
                                        ),
                                    ],
                                    form.span,
                                ),
                            ],
                            form.span,
                        ));
                    }
                    loop_body.extend(items[body_start..].iter().cloned());
                    let mut let_items = vec![
                        Form::atom("LET", form.span),
                        Form::list(
                            vec![Form::list(
                                vec![
                                    variable,
                                    Form::list(
                                        vec![
                                            Form::atom("AREF", form.span),
                                            vector.clone(),
                                            index.clone(),
                                        ],
                                        form.span,
                                    ),
                                ],
                                form.span,
                            )],
                            form.span,
                        ),
                    ];
                    let_items.extend(loop_body);
                    let dotimes = Form::list(
                        vec![
                            Form::atom("DOTIMES", form.span),
                            Form::list(
                                vec![
                                    index,
                                    Form::list(
                                        vec![Form::atom("LENGTH", form.span), vector.clone()],
                                        form.span,
                                    ),
                                ],
                                form.span,
                            ),
                            Form::list(let_items, form.span),
                        ],
                        form.span,
                    );
                    let vector_loop = Form::list(
                        vec![
                            Form::atom("LET", form.span),
                            Form::list(
                                vec![Form::list(
                                    vec![vector.clone(), items[4].clone()],
                                    form.span,
                                )],
                                form.span,
                            ),
                            dotimes,
                        ],
                        form.span,
                    );
                    if collect_form.is_some()
                        || sum_name.is_some()
                        || count_name.is_some()
                        || extremum_name.is_some()
                    {
                        let mut bindings = Vec::new();
                        if collect_form.is_some() {
                            bindings.push(Form::list(
                                vec![collect_name.clone(), Form::atom("NIL", form.span)],
                                form.span,
                            ));
                        }
                        if let Some(name) = sum_name.clone() {
                            bindings.push(Form::list(
                                vec![name, Form::atom("0", form.span)],
                                form.span,
                            ));
                        }
                        if let Some(name) = count_name.clone() {
                            bindings.push(Form::list(
                                vec![name, Form::atom("0", form.span)],
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
                        } else if let Some(name) = count_name {
                            name
                        } else {
                            extremum_name.expect("aggregate name is present")
                        };
                        return Ok(Form::list(
                            vec![
                                Form::atom("LET", form.span),
                                Form::list(bindings, form.span),
                                Form::list(
                                    vec![Form::atom("PROGN", form.span), vector_loop, result],
                                    form.span,
                                ),
                            ],
                            form.span,
                        ));
                    }
                    return Ok(vector_loop);
                }
                if items
                    .get(3)
                    .and_then(atom_name)
                    .is_some_and(|name| names_equal(name, "="))
                {
                    if items.len() < 7
                        || !items
                            .get(5)
                            .and_then(atom_name)
                            .is_some_and(|name| names_equal(name, "THEN"))
                    {
                        return Err(Self::invalid(
                            "LOOP FOR = requires a variable, initial value, and THEN step",
                            form.span,
                        ));
                    }
                    let variable = items[2].clone();
                    let mut body_start = 7;
                    let count = if items
                        .get(body_start)
                        .and_then(atom_name)
                        .is_some_and(|name| names_equal(name, "REPEAT"))
                    {
                        if items.len() <= body_start + 1 {
                            return Err(Self::invalid(
                                "LOOP REPEAT clause requires a count",
                                form.span,
                            ));
                        }
                        let count = items[body_start + 1].clone();
                        body_start += 2;
                        Some(count)
                    } else {
                        None
                    };
                    let termination = if count.is_some() {
                        Form::list(
                            vec![Form::list(
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
                            )],
                            form.span,
                        )
                    } else if items
                        .get(body_start)
                        .and_then(atom_name)
                        .is_some_and(|name| {
                            names_equal(name, "WHILE") || names_equal(name, "UNTIL")
                        })
                    {
                        if items.len() <= body_start + 1 {
                            return Err(Self::invalid(
                                "LOOP condition clause requires a test",
                                form.span,
                            ));
                        }
                        let stop_on_true = items
                            .get(body_start)
                            .and_then(atom_name)
                            .is_some_and(|name| names_equal(name, "UNTIL"));
                        let guard_operator = if stop_on_true { "WHEN" } else { "UNLESS" };
                        body_start += 2;
                        Form::list(
                            vec![Form::list(
                                vec![
                                    Form::atom(guard_operator, form.span),
                                    items[body_start - 1].clone(),
                                    Form::list(vec![Form::atom("RETURN", form.span)], form.span),
                                ],
                                form.span,
                            )],
                            form.span,
                        )
                    } else {
                        return Err(Self::invalid(
                            "LOOP FOR = requires REPEAT, WHILE, or UNTIL",
                            form.span,
                        ));
                    };
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
                    let count_name = count_name.clone();
                    let mut do_items = vec![
                        Form::atom("DO", form.span),
                        Form::list(
                            vec![Form::list(
                                vec![variable, items[4].clone(), items[6].clone()],
                                form.span,
                            )],
                            form.span,
                        ),
                        termination,
                    ];
                    if let Some(value) = collect_form.clone() {
                        do_items.push(Form::list(
                            vec![Form::atom("PUSH", form.span), value, collect_name.clone()],
                            form.span,
                        ));
                    }
                    do_items.extend(items[body_start..].iter().cloned());
                    if count.is_some() {
                        do_items.push(Form::list(
                            vec![Form::atom("DECF", form.span), count_name.clone()],
                            form.span,
                        ));
                    }
                    let do_form = Form::list(do_items, form.span);
                    let mut bindings = vec![];
                    if let Some(count) = count {
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
                        Form::atom("NIL", form.span)
                    };
                    return Ok(Form::list(
                        vec![
                            Form::atom("LET", form.span),
                            Form::list(bindings, form.span),
                            Form::list(
                                vec![Form::atom("PROGN", form.span), do_form, result],
                                form.span,
                            ),
                        ],
                        form.span,
                    ));
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
                let mut count_form = None;
                let mut count_name = None;
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
                            || names_equal(name, "COUNT")
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
                    } else if names_equal(aggregate_clause, "COUNT") {
                        count_form = Some(items[body_start + 1].clone());
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
                    } else if count_form.is_some() {
                        count_name = Some(aggregate_name);
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
                    do_items.push(sum_step(form, value, name));
                }
                if let (Some(value), Some(name)) = (count_form, count_name.clone()) {
                    do_items.push(count_step(form, value, name));
                }
                if let (Some(value), Some(name)) = (extremum_form, extremum_name.clone()) {
                    let comparison = if maximize { ">" } else { "<" };
                    let candidate =
                        Form::atom(format!("NCL-LOOP-CANDIDATE-{}", form.span.start), form.span);
                    let body = Form::list(
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
                                            candidate.clone(),
                                            name.clone(),
                                        ],
                                        form.span,
                                    ),
                                ],
                                form.span,
                            ),
                            Form::list(
                                vec![Form::atom("SETQ", form.span), name, candidate.clone()],
                                form.span,
                            ),
                        ],
                        form.span,
                    );
                    do_items.push(Form::list(
                        vec![
                            Form::atom("LET", form.span),
                            Form::list(
                                vec![Form::list(vec![candidate, value], form.span)],
                                form.span,
                            ),
                            body,
                        ],
                        form.span,
                    ));
                }
                do_items.extend(items[body_start..].iter().cloned());
                let do_form = Form::list(do_items, form.span);
                if collect_form.is_some()
                    || sum_name.is_some()
                    || count_name.is_some()
                    || extremum_name.is_some()
                {
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
                    if let Some(name) = count_name.clone() {
                        bindings.push(Form::list(
                            vec![name, Form::atom("0", form.span)],
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
                    } else if let Some(name) = count_name {
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
