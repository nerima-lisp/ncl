use ncl_syntax::Form;

use crate::{evaluator::helpers::atom_name, environment::names_equal, Runtime, RuntimeError};

pub(super) fn named_loop_body_start(
    form: &Form,
    items: &[Form],
) -> Result<(Option<Form>, usize), RuntimeError> {
    let named_block = if items
        .get(1)
        .and_then(atom_name)
        .is_some_and(|name| names_equal(name, "NAMED"))
    {
        let Some(name) = items.get(2).and_then(atom_name) else {
            return Err(Runtime::invalid("LOOP NAMED requires a name", form.span));
        };
        Some(Form::atom(name, form.span))
    } else {
        None
    };
    let body_start = usize::from(named_block.is_some()) * 2 + 1;
    Ok((named_block, body_start))
}

impl Runtime {
    pub(super) fn expand_loop_return_clause(
        form: &Form,
        items: &[Form],
        offset: usize,
    ) -> Result<Form, RuntimeError> {
        let Some(return_value) = items.get(offset + 1) else {
            return Err(Self::invalid("LOOP RETURN clause requires a form", form.span));
        };
        let mut core_items = items[..offset].to_vec();
        core_items.push(Form::list(
            vec![Form::atom("RETURN", form.span), return_value.clone()],
            form.span,
        ));
        core_items.extend(items[offset + 2..].iter().cloned());
        Self::expand_builtin_loop(&Form::list(core_items, form.span))
    }

    pub(super) fn expand_loop_initially_clause(
        form: &Form,
        items: &[Form],
        offset: usize,
    ) -> Result<Form, RuntimeError> {
        let Some(initially_form) = items.get(offset + 1) else {
            return Err(Self::invalid(
                "LOOP INITIALLY clause requires a form",
                form.span,
            ));
        };
        let mut core_items = items[..offset].to_vec();
        core_items.extend(items[offset + 2..].iter().cloned());
        let expanded = Self::expand_builtin_loop(&Form::list(core_items, form.span))?;
        Ok(Form::list(
            vec![
                Form::atom("PROGN", form.span),
                initially_form.clone(),
                expanded,
            ],
            form.span,
        ))
    }

    pub(super) fn expand_loop_finally_clause(
        form: &Form,
        items: &[Form],
        offset: usize,
    ) -> Result<Form, RuntimeError> {
        let finally_items = &items[offset + 1..];
        if finally_items.is_empty() {
            return Err(Self::invalid(
                "LOOP FINALLY clause requires a form",
                form.span,
            ));
        }
        let core_form = Form::list(items[..offset].to_vec(), form.span);
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
        Ok(Form::list(
            vec![Form::atom("PROGN", form.span), expanded, finally_form],
            form.span,
        ))
    }
}

pub(super) fn clause_offset(items: &[Form], name: &str) -> Option<usize> {
    items.iter().position(|item| {
        atom_name(item).is_some_and(|item_name| names_equal(item_name, name))
    })
}
