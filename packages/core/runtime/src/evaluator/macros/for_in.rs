use ncl_syntax::Form;

use crate::{environment::names_equal, evaluator::helpers::atom_name, Runtime, RuntimeError};

use super::loop_aggregate::{append_step, count_step, sum_step};
use super::loop_hash::bind_hash_value_and_key;

pub(super) fn expand_loop_for_in(form: &Form, items: &[Form]) -> Result<Option<Form>, RuntimeError> {
    if !items
        .get(3)
        .and_then(atom_name)
        .is_some_and(|name| names_equal(name, "IN"))
    {
        return Ok(None);
    }
    let mut collect_form = None;
    let mut collect_name =
        Form::atom(format!("NCL-LOOP-COLLECT-{}", form.span.start), form.span);
    if items.len() < 5 {
        return Err(Runtime::invalid(
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
            return Err(Runtime::invalid(
                "LOOP hash binding requires a variable and table",
                form.span,
            ));
        }
        let binding =
            (items[body_start + 1].clone(), items[body_start + 2].clone());
        body_start += 3;
        Some(binding)
    } else if items
        .get(body_start)
        .and_then(atom_name)
        .is_some_and(|name| names_equal(name, "NCL-HASH-BIND2"))
    {
        if items.len() <= body_start + 3 {
            return Err(Runtime::invalid(
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
            return Err(Runtime::invalid(
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
                return Err(Runtime::invalid(
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
            return Err(Runtime::invalid(
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
                return Err(Runtime::invalid(
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
    let mut loop_condition = None;
    if let Some(condition_name) = items.get(body_start).and_then(atom_name) {
        if names_equal(condition_name, "THEREIS")
            || names_equal(condition_name, "ALWAYS")
            || names_equal(condition_name, "NEVER")
        {
            if items.len() <= body_start + 1 {
                return Err(Runtime::invalid(
                    "LOOP condition clause requires a form",
                    form.span,
                ));
            }
            loop_condition =
                Some((condition_name.to_owned(), items[body_start + 1].clone()));
            body_start += 2;
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
    if let Some((condition_name, condition)) = loop_condition.clone() {
        let success = names_equal(&condition_name, "THEREIS");
        let predicate = if names_equal(&condition_name, "THEREIS") {
            condition.clone()
        } else if names_equal(&condition_name, "ALWAYS") {
            Form::list(
                vec![Form::atom("NOT", form.span), condition.clone()],
                form.span,
            )
        } else {
            condition.clone()
        };
        let value = if success {
            condition
        } else {
            Form::atom("NIL", form.span)
        };
        dolist_items.push(Form::list(
            vec![
                Form::atom("WHEN", form.span),
                predicate,
                Form::list(
                    vec![
                        Form::atom("RETURN-FROM", form.span),
                        Form::atom("NIL", form.span),
                        value,
                    ],
                    form.span,
                ),
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
    let dolist = if let Some((condition_name, _)) = &loop_condition {
        let normal_result = if names_equal(condition_name, "THEREIS") {
            Form::atom("NIL", form.span)
        } else {
            Form::atom("T", form.span)
        };
        Form::list(
            vec![
                Form::atom("BLOCK", form.span),
                Form::atom("NIL", form.span),
                Form::list(
                    vec![Form::atom("PROGN", form.span), dolist, normal_result],
                    form.span,
                ),
            ],
            form.span,
        )
    } else {
        dolist
    };
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
        return Ok(Some(Form::list(
            vec![
                Form::atom("LET", form.span),
                Form::list(bindings, form.span),
                body_result,
            ],
            form.span,
        )));
    }
    Ok(Some(dolist))
}
