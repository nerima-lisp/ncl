#![allow(clippy::wildcard_imports)]
use super::*;

mod assignment;
mod assignment_tests;
mod binding_forms;
mod binding_forms_tests;
mod clauses;
mod clauses_tests;
mod dispatch;
mod dispatch_tests;
mod lambda;
mod lambda_tests;
mod local_macros;
mod local_macros_tests;
mod quasiquote;
mod reservations;
mod reservations_tests;
mod setf;
mod special_forms;
mod special_forms_tests;
mod symbol_macro;
mod symbol_macro_tests;

impl Runtime {
    pub(super) fn prepare_compiled_top_level_form(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        if is_operator_form(form, "DEFPACKAGE") || is_operator_form(form, "IN-PACKAGE") {
            let value = self.eval_values_in(form, environment)?;
            return Self::quoted_value_form(&value, form.span);
        }

        self.prepare_compiled_form(form, environment)
    }

    pub(super) fn prepare_compiled_form(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let mut reservations = reservations::CompilationReservations::new(self);
        reservations.reserve(form);
        if let Some(expanded) = Self::expand_symbol_macro_form(form, environment)? {
            return self.prepare_compiled_form(&expanded, environment);
        }

        if is_operator_form(form, "MACROLET") {
            return self.prepare_compiled_macrolet(form, environment);
        }
        if is_operator_form(form, "SYMBOL-MACROLET") {
            return self.prepare_compiled_symbol_macrolet(form, environment);
        }
        if is_operator_form(form, "WITH-OPEN-FILE") {
            let expanded = self.expand_with_open_file(form)?;
            return self.prepare_compiled_form(&expanded, environment);
        }
        if is_operator_form(form, "WITH-OPEN-STREAM") {
            let expanded = self.expand_with_open_stream(form)?;
            return self.prepare_compiled_form(&expanded, environment);
        }

        if is_operator_form(form, "LOAD-TIME-VALUE") {
            let value = self.eval_values_in(form, environment)?.primary_value();
            return Ok(Form::new(
                FormKind::Literal(ncl_syntax::OpaqueLiteral::new(value)),
                form.span,
            ));
        }

        if is_operator_form(form, "DEFMACRO")
            || is_operator_form(form, "DEFINE-MODIFY-MACRO")
            || is_operator_form(form, "DEFINE-SETF-EXPANDER")
            || is_operator_form(form, "DEFINE-SYMBOL-MACRO")
            || is_operator_form(form, "MACROEXPAND-1")
            || is_operator_form(form, "MACROEXPAND")
        {
            let value = self.eval_values_in(form, environment)?;
            return Self::quoted_value_form(&value, form.span);
        }

        let (expanded, changed) = self.expand_macros_with_flag(form.clone(), environment)?;
        reservations.reserve(&expanded);
        if changed {
            return self.prepare_compiled_form(&expanded, environment);
        }
        match &expanded.kind {
            FormKind::List(items) => self.prepare_compiled_list(&expanded, items, environment),
            _ => Ok(expanded),
        }
    }

    fn prepare_tail(
        &self,
        items: &mut [Form],
        start: usize,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        for item in items.iter_mut().skip(start) {
            *item = self.prepare_compiled_form(item, environment)?;
        }
        Ok(())
    }

    fn quoted_value_form(value: &Value, span: Span) -> Result<Form, RuntimeError> {
        if let Value::Values(values) = value {
            let mut forms = vec![Form::atom("VALUES", span)];
            for value in values.iter() {
                forms.push(Self::quoted_value_form(value, span)?);
            }
            return Ok(Form::list(forms, span));
        }

        Ok(Self::retained_value_form(value, span))
    }
}
