#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn prepare_compiled_setq(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            let mut prepared = items.to_vec();
            self.prepare_tail(&mut prepared, 2, environment)?;
            return Ok(Form::list(prepared, form.span));
        }

        let expansions = items[1..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| Self::expand_symbol_macro_form(&pair[0], environment))
            .collect::<Result<Vec<_>, _>>()?;
        if !expansions.iter().any(Option::is_some) {
            let mut prepared = items.to_vec();
            for index in (2..prepared.len()).step_by(2) {
                prepared[index] = self.prepare_compiled_form(&items[index], environment)?;
            }
            return Ok(Form::list(prepared, form.span));
        }

        let mut transformed = vec![Form::atom("PROGN", form.span)];
        for (pair, expansion) in items[1..].as_chunks::<2>().0.iter().zip(expansions) {
            let is_symbol_macro = expansion.is_some();
            let target = expansion.unwrap_or_else(|| pair[0].clone());
            let operator = if is_symbol_macro { "SETF" } else { "SETQ" };
            let assignment = Form::list(
                vec![Form::atom(operator, pair[0].span), target, pair[1].clone()],
                pair[0].span,
            );
            transformed.push(self.prepare_compiled_form(&assignment, environment)?);
        }
        Ok(Form::list(transformed, form.span))
    }

    pub(super) fn prepare_compiled_psetq(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            let mut prepared = items.to_vec();
            self.prepare_tail(&mut prepared, 2, environment)?;
            return Ok(Form::list(prepared, form.span));
        }

        let expansions = items[1..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| Self::expand_symbol_macro_form(&pair[0], environment))
            .collect::<Result<Vec<_>, _>>()?;
        if !expansions.iter().any(Option::is_some) {
            let mut prepared = items.to_vec();
            for index in (2..prepared.len()).step_by(2) {
                prepared[index] = self.prepare_compiled_form(&items[index], environment)?;
            }
            return Ok(Form::list(prepared, form.span));
        }

        let mut bindings = Vec::with_capacity(expansions.len());
        let mut body = vec![Form::atom("PROGN", form.span)];
        for (index, (pair, expansion)) in items[1..]
            .as_chunks::<2>()
            .0
            .iter()
            .zip(expansions)
            .enumerate()
        {
            let temporary = Self::symbol_macro_temporary(form, index, pair[0].span);
            bindings.push(Form::list(
                vec![temporary.clone(), pair[1].clone()],
                pair[0].span,
            ));
            let is_symbol_macro = expansion.is_some();
            let target = expansion.unwrap_or_else(|| pair[0].clone());
            let operator = if is_symbol_macro { "SETF" } else { "SETQ" };
            body.push(Form::list(
                vec![Form::atom(operator, pair[0].span), target, temporary],
                pair[0].span,
            ));
        }
        body.push(Form::atom("NIL", form.span));

        let mut transformed = vec![
            Form::atom("LET", form.span),
            Form::list(bindings, form.span),
        ];
        transformed.push(Form::list(body, form.span));
        self.prepare_compiled_form(&Form::list(transformed, form.span), environment)
    }
}
