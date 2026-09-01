use ncl_syntax::{Form, FormKind};

use crate::{environment::names_equal, evaluator::helpers::atom_name, Runtime, RuntimeError};

use super::loop_aggregate::{append_step, count_step, sum_step};
use super::loop_hash::bind_hash_value_and_key;
use super::loop_on::expand_loop_for_on;

impl Runtime {
    pub(super) fn expand_builtin_loop(form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if let Some(return_offset) = items.iter().position(|item| {
            atom_name(item).is_some_and(|name| names_equal(name, "RETURN"))
        }) {
            let Some(return_value) = items.get(return_offset + 1) else {
                return Err(Self::invalid("LOOP RETURN clause requires a form", form.span));
            };
            let mut core_items = items[..return_offset].to_vec();
            core_items.push(Form::list(
                vec![Form::atom("RETURN", form.span), return_value.clone()],
                form.span,
            ));
            core_items.extend(items[return_offset + 2..].iter().cloned());
            return Self::expand_builtin_loop(&Form::list(core_items, form.span));
        }
        if let Some(initially_offset) = items.iter().position(|item| {
            atom_name(item).is_some_and(|name| names_equal(name, "INITIALLY"))
        }) {
            let Some(initially_form) = items.get(initially_offset + 1) else {
                return Err(Self::invalid(
                    "LOOP INITIALLY clause requires a form",
                    form.span,
                ));
            };
            let mut core_items = items[..initially_offset].to_vec();
            core_items.extend(items[initially_offset + 2..].iter().cloned());
            let core_form = Form::list(core_items, form.span);
            let expanded = Self::expand_builtin_loop(&core_form)?;
            return Ok(Form::list(
                vec![
                    Form::atom("PROGN", form.span),
                    initially_form.clone(),
                    expanded,
                ],
                form.span,
            ));
        }
        if let Some(finally_offset) = items.iter().position(|item| {
            atom_name(item).is_some_and(|name| names_equal(name, "FINALLY"))
        }) {
            let finally_items = &items[finally_offset + 1..];
            if finally_items.is_empty() {
                return Err(Self::invalid(
                    "LOOP FINALLY clause requires a form",
                    form.span,
                ));
            }
            let core_form = Form::list(items[..finally_offset].to_vec(), form.span);
            let expanded = Self::expand_builtin_loop(&core_form)?;
            let finally_form = if finally_items.len() == 1 {
                finally_items[0].clone()
            } else {
                Form::list(
                    std::iter::once(Form::atom("PROGN", form.span))
                        .chain(finally_items.iter().cloned())
                        .collect(),
                    form.span,
                )
            };
            return Ok(Form::list(
                vec![Form::atom("PROGN", form.span), expanded, finally_form],
                form.span,
            ));
        }
        let mut body = items[1..].to_vec();
        if body.first().and_then(atom_name).is_some_and(|name| names_equal(name, "DO")) {
            body.remove(0);
        }
        let mut repeat_count = None;
        let mut collect_form = None;
        let mut finally_form = None;
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
                if let Some(finally_offset) = body.iter().position(|item| {
                    atom_name(item).is_some_and(|name| names_equal(name, "FINALLY"))
                }) {
                    let finally_items = body.split_off(finally_offset + 1);
                    body.pop();
                    if finally_items.is_empty() {
                        return Err(Self::invalid(
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
                    .is_some_and(|name| names_equal(name, "BEING"))
                {
                    if items.len() < 8
                        || !items
                            .get(4)
                            .and_then(atom_name)
                            .is_some_and(|name| names_equal(name, "THE"))
                        || !items
                            .get(6)
                            .and_then(atom_name)
                            .is_some_and(|name| names_equal(name, "OF"))
                    {
                        return Err(Self::invalid(
                            "LOOP FOR BEING requires THE HASH-KEYS/HASH-VALUES OF and a table",
                            form.span,
                        ));
                    }
                    let Some(kind) = items.get(5).and_then(atom_name) else {
                        return Err(Self::invalid(
                            "LOOP FOR BEING requires HASH-KEYS or HASH-VALUES",
                            form.span,
                        ));
                    };
                    let iterator = if names_equal(kind, "HASH-KEYS") {
                        "NCL-HASH-TABLE-KEYS"
                    } else if names_equal(kind, "HASH-VALUES") {
                        "NCL-HASH-TABLE-VALUES"
                    } else {
                        return Err(Self::invalid(
                            "LOOP FOR BEING requires HASH-KEYS or HASH-VALUES",
                            form.span,
                        ));
                    };
                    let mut using_binding = None;
                    if items
                        .get(8)
                        .and_then(atom_name)
                        .is_some_and(|name| names_equal(name, "USING"))
                    {
                        let Some(using_form) = items.get(9) else {
                            return Err(Self::invalid(
                                "LOOP HASH-TABLE USING requires a binding",
                                form.span,
                            ));
                        };
                        let FormKind::List(using_items) = &using_form.kind else {
                            return Err(Self::invalid(
                                "LOOP HASH-TABLE USING requires (HASH-KEY/HASH-VALUE variable)",
                                form.span,
                            ));
                        };
                        if using_items.len() != 2 {
                            return Err(Self::invalid(
                                "LOOP HASH-TABLE USING requires (HASH-KEY/HASH-VALUE variable)",
                                form.span,
                            ));
                        }
                        let Some(binding_kind) = atom_name(&using_items[0]) else {
                            return Err(Self::invalid(
                                "LOOP HASH-TABLE USING requires HASH-KEY or HASH-VALUE",
                                form.span,
                            ));
                        };
                        if names_equal(kind, "HASH-KEYS")
                            && !names_equal(binding_kind, "HASH-VALUE")
                        {
                            return Err(Self::invalid(
                                "LOOP HASH-TABLE USING binding does not match iterator",
                                form.span,
                            ));
                        }
                        if names_equal(kind, "HASH-VALUES") {
                            let internal_key = Form::atom(
                                format!("NCL-HASH-KEY-{}", form.span.start),
                                form.span,
                            );
                            let mut rewritten = vec![
                                items[0].clone(),
                                Form::atom("FOR", form.span),
                                internal_key,
                                Form::atom("IN", form.span),
                                Form::list(
                                    vec![
                                        Form::atom("NCL-HASH-TABLE-KEYS", form.span),
                                        items[7].clone(),
                                    ],
                                    form.span,
                                ),
                                Form::atom("NCL-HASH-BIND2", form.span),
                                items[2].clone(),
                                using_items[1].clone(),
                                items[7].clone(),
                            ];
                            rewritten.extend(items[10..].iter().cloned());
                            return Self::expand_builtin_loop(&Form::list(rewritten, form.span));
                        }
                        using_binding = Some(using_items[1].clone());
                    }
                    let mut rewritten = vec![
                        items[0].clone(),
                        Form::atom("FOR", form.span),
                        items[2].clone(),
                        Form::atom("IN", form.span),
                        Form::list(
                            vec![Form::atom(iterator, form.span), items[7].clone()],
                            form.span,
                        ),
                    ];
                    if let Some(binding) = using_binding {
                        rewritten.extend([
                            Form::atom("NCL-HASH-BIND", form.span),
                            binding,
                            items[7].clone(),
                        ]);
                        rewritten.extend(items[10..].iter().cloned());
                    } else {
                        rewritten.extend(items[8..].iter().cloned());
                    }
                    return Self::expand_builtin_loop(&Form::list(rewritten, form.span));
                } else if items
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
                    let mut hash_dual_binding = None;
                    let hash_binding = if items
                        .get(body_start)
                        .and_then(atom_name)
                        .is_some_and(|name| names_equal(name, "NCL-HASH-BIND"))
                    {
                        if items.len() <= body_start + 2 {
                            return Err(Self::invalid(
                                "LOOP hash binding requires a variable and table",
                                form.span,
                            ));
                        }
                        let binding = (items[body_start + 1].clone(), items[body_start + 2].clone());
                        body_start += 3;
                        Some(binding)
                    } else if items
                        .get(body_start)
                        .and_then(atom_name)
                        .is_some_and(|name| names_equal(name, "NCL-HASH-BIND2"))
                    {
                        if items.len() <= body_start + 3 {
                            return Err(Self::invalid(
                                "LOOP hash dual binding requires value, key, and table",
                                form.span,
                            ));
                        }
                        hash_dual_binding = Some((
                            items[body_start + 1].clone(),
                            items[body_start + 2].clone(),
                            items[body_start + 3].clone(),
                        ));
                        body_start += 4;
                        None
                    } else {
                        None
                    };
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
                            names_equal(name, "COLLECT")
                                || names_equal(name, "APPEND")
                                || names_equal(name, "NCONC")
                        })
                    {
                        if items.len() <= body_start + 1 {
                            return Err(Self::invalid(
                                "LOOP COLLECT clause requires a form",
                                form.span,
                            ));
                        }
                        collect_form = Some(items[body_start + 1].clone());
                        append = names_equal(atom_name(&items[body_start]).unwrap(), "APPEND")
                            || names_equal(atom_name(&items[body_start]).unwrap(), "NCONC");
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
                        Form::list(vec![variable.clone(), items[4].clone()], form.span),
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
                    if let Some((binding, table)) = hash_binding {
                        for item in &mut dolist_items[2..] {
                            *item = Form::list(
                                vec![
                                    Form::atom("LET", form.span),
                                    Form::list(
                                        vec![Form::list(
                                            vec![
                                                binding.clone(),
                                                Form::list(
                                                    vec![
                                                        Form::atom("GETHASH", form.span),
                                                        variable.clone(),
                                                        table.clone(),
                                                    ],
                                                    form.span,
                                                ),
                                            ],
                                            form.span,
                                        )],
                                        form.span,
                                    ),
                                    item.clone(),
                                ],
                                form.span,
                            );
                        }
                    }
                    if let Some((value_binding, key_binding, table)) = hash_dual_binding {
                        for item in &mut dolist_items[2..] {
                            *item = bind_hash_value_and_key(
                                form,
                                &value_binding,
                                &key_binding,
                                &variable,
                                &table,
                                item.clone(),
                            );
                        }
                    }
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
                if let Some(expanded) = expand_loop_for_on(form, items)? {
                    return Ok(expanded);
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
                    let mut append_form = None;
                    if items
                        .get(body_start)
                        .and_then(atom_name)
                        .is_some_and(|name| {
                            names_equal(name, "COLLECT")
                                || names_equal(name, "APPEND")
                                || names_equal(name, "NCONC")
                        })
                    {
                        if items.len() <= body_start + 1 {
                            return Err(Self::invalid(
                                "LOOP COLLECT clause requires a form",
                                form.span,
                            ));
                        }
                        collect_form = Some(items[body_start + 1].clone());
                        let append = names_equal(atom_name(&items[body_start]).unwrap(), "APPEND")
                            || names_equal(atom_name(&items[body_start]).unwrap(), "NCONC");
                        if append {
                            append_form = Some(items[body_start + 1].clone());
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
                    if let Some(value) = append_form {
                        loop_body.push(append_step(form, value, collect_name.clone()));
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
            let block_result = if let Some(finally) = finally_form {
                Form::list(
                    vec![Form::atom("PROGN", form.span), block_result, finally],
                    form.span,
                )
            } else {
                block_result
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
