#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    fn symbol_macro_expansion_for_atom(atom: &str, environment: &Environment) -> Option<Form> {
        if literal_atom(atom).is_some() {
            return None;
        }

        let (name, escaped) = resolved_symbol(atom);
        if escaped {
            environment.lookup_symbol_macro_exact(&name)
        } else {
            environment.lookup_symbol_macro(&name)
        }
    }

    pub(crate) fn expand_symbol_macro_form(
        form: &Form,
        environment: &Environment,
    ) -> Result<Option<Form>, RuntimeError> {
        let mut current = form.clone();
        let mut expanded = false;
        let mut seen = HashSet::new();

        loop {
            let Some(atom) = atom_name(&current) else {
                return Ok(if expanded { Some(current) } else { None });
            };
            let Some(next) = Self::symbol_macro_expansion_for_atom(atom, environment) else {
                return Ok(if expanded { Some(current) } else { None });
            };
            let (name, escaped) = resolved_symbol(atom);
            let key = format!("{}:{}", if escaped { "escaped" } else { "normal" }, name);
            if !seen.insert(key) {
                return Err(Self::invalid("recursive symbol macro expansion", form.span));
            }
            expanded = true;
            current = next;
        }
    }

    pub(crate) fn symbol_macro_temporary(form: &Form, index: usize, span: Span) -> Form {
        Form::atom(
            format!(
                "NCL-SYMBOL-MACRO-TEMP-{}-{}-{}",
                form.span.start, form.span.end, index
            ),
            span,
        )
    }

    pub(super) fn prepare_compiled_multiple_value_setq(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let Some(variable_form) = items.get(1) else {
            let mut prepared = items.to_vec();
            self.prepare_tail(&mut prepared, 2, environment)?;
            return Ok(Form::list(prepared, form.span));
        };
        let FormKind::List(variable_forms) = &variable_form.kind else {
            let mut prepared = items.to_vec();
            if prepared.len() > 2 {
                prepared[2] = self.prepare_compiled_form(&items[2], environment)?;
            }
            return Ok(Form::list(prepared, form.span));
        };

        let expansions = variable_forms
            .iter()
            .map(|variable| Self::expand_symbol_macro_form(variable, environment))
            .collect::<Result<Vec<_>, _>>()?;
        if !expansions.iter().any(Option::is_some) {
            let mut prepared = items.to_vec();
            if prepared.len() > 2 {
                prepared[2] = self.prepare_compiled_form(&items[2], environment)?;
            }
            return Ok(Form::list(prepared, form.span));
        }

        let temporaries = variable_forms
            .iter()
            .enumerate()
            .map(|(index, variable)| Self::symbol_macro_temporary(form, index, variable.span))
            .collect::<Vec<_>>();
        let mut body = Vec::with_capacity(variable_forms.len() + 1);
        for ((variable, expansion), temporary) in variable_forms
            .iter()
            .zip(expansions)
            .zip(temporaries.iter())
        {
            let is_symbol_macro = expansion.is_some();
            let target = expansion.unwrap_or_else(|| variable.clone());
            let operator = if is_symbol_macro { "SETF" } else { "SETQ" };
            body.push(Form::list(
                vec![
                    Form::atom(operator, variable.span),
                    target,
                    temporary.clone(),
                ],
                variable.span,
            ));
        }
        body.push(temporaries[0].clone());

        let mut transformed = vec![
            Form::atom("MULTIPLE-VALUE-BIND", form.span),
            Form::list(temporaries, variable_form.span),
            items[2].clone(),
        ];
        transformed.extend(body);
        self.prepare_compiled_form(&Form::list(transformed, form.span), environment)
    }
}
