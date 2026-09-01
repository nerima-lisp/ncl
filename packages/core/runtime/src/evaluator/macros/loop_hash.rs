use ncl_syntax::{Form, FormKind};

use crate::{environment::names_equal, evaluator::helpers::atom_name, Runtime, RuntimeError};

pub(super) fn expand_loop_hash_being(
    form: &Form,
    items: &[Form],
) -> Result<Option<Form>, RuntimeError> {
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
        return Err(Runtime::invalid(
            "LOOP FOR BEING requires THE HASH-KEYS/HASH-VALUES OF and a table",
            form.span,
        ));
    }
    let Some(kind) = items.get(5).and_then(atom_name) else {
        return Err(Runtime::invalid(
            "LOOP FOR BEING requires HASH-KEYS or HASH-VALUES",
            form.span,
        ));
    };
    let Some(iterator) = hash_iterator_name(kind) else {
        return Err(Runtime::invalid(
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
            return Err(Runtime::invalid(
                "LOOP HASH-TABLE USING requires a binding",
                form.span,
            ));
        };
        let FormKind::List(using_items) = &using_form.kind else {
            return Err(Runtime::invalid(
                "LOOP HASH-TABLE USING requires (HASH-KEY/HASH-VALUE variable)",
                form.span,
            ));
        };
        if using_items.len() != 2 {
            return Err(Runtime::invalid(
                "LOOP HASH-TABLE USING requires (HASH-KEY/HASH-VALUE variable)",
                form.span,
            ));
        }
        let Some(binding_kind) = atom_name(&using_items[0]) else {
            return Err(Runtime::invalid(
                "LOOP HASH-TABLE USING requires HASH-KEY or HASH-VALUE",
                form.span,
            ));
        };
        if names_equal(kind, "HASH-KEYS") && !names_equal(binding_kind, "HASH-VALUE") {
            return Err(Runtime::invalid(
                "LOOP HASH-TABLE USING binding does not match iterator",
                form.span,
            ));
        }
        if names_equal(kind, "HASH-VALUES") {
            let internal_key = Form::atom(format!("NCL-HASH-KEY-{}", form.span.start), form.span);
            let mut rewritten = vec![
                items[0].clone(), Form::atom("FOR", form.span), internal_key,
                Form::atom("IN", form.span),
                Form::list(vec![Form::atom("NCL-HASH-TABLE-KEYS", form.span), items[7].clone()], form.span),
                Form::atom("NCL-HASH-BIND2", form.span), items[2].clone(),
                using_items[1].clone(), items[7].clone(),
            ];
            rewritten.extend(items[10..].iter().cloned());
            return Ok(Some(Runtime::expand_builtin_loop(&Form::list(rewritten, form.span))?));
        }
        using_binding = Some(using_items[1].clone());
    }
    let mut rewritten = vec![
        items[0].clone(), Form::atom("FOR", form.span), items[2].clone(),
        Form::atom("IN", form.span),
        Form::list(vec![Form::atom(iterator, form.span), items[7].clone()], form.span),
    ];
    if let Some(binding) = using_binding {
        rewritten.extend([Form::atom("NCL-HASH-BIND", form.span), binding, items[7].clone()]);
        rewritten.extend(items[10..].iter().cloned());
    } else {
        rewritten.extend(items[8..].iter().cloned());
    }
    Ok(Some(Runtime::expand_builtin_loop(&Form::list(rewritten, form.span))?))
}

pub(super) fn hash_iterator_name(kind: &str) -> Option<&'static str> {
    if kind.eq_ignore_ascii_case("HASH-KEYS") {
        Some("NCL-HASH-TABLE-KEYS")
    } else if kind.eq_ignore_ascii_case("HASH-VALUES") {
        Some("NCL-HASH-TABLE-VALUES")
    } else {
        None
    }
}

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
