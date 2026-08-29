#[cfg(test)]
use super::Form;
use super::{Environment, Runtime, RuntimeError, SetfExpansion, Span, Value};

impl Runtime {
    pub(in crate::evaluator::evaluator_special_forms) fn apply_setf_expansion(
        &self,
        expansion: &SetfExpansion,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if expansion.temporaries.len() != expansion.values.len() {
            return Err(Self::invalid(
                "SETF expansion temporary and value lists must have the same length",
                span,
            ));
        }
        let local = environment.child();
        for (temporary, value_form) in expansion.temporaries.iter().zip(&expansion.values) {
            let (name, escaped) =
                Self::variable_name_info(temporary, "SETF temporary must be a symbol")?;
            let value = self.eval_in(value_form, &local)?;
            self.define_variable_in(&name, escaped, value, &local);
        }
        let (store_name, store_escaped) =
            Self::variable_name_info(&expansion.store, "SETF store variable must be a symbol")?;
        self.define_variable_in(&store_name, store_escaped, value, &local);
        self.eval_in(&expansion.store_form, &local)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_setf_expansion_rejects_mismatched_temporary_and_value_lists() {
        let runtime = Runtime::new();
        let environment = runtime.global_environment();
        let span = Span::new(0, 0);
        let expansion = SetfExpansion {
            temporaries: vec![Form::atom("NCL-SETF-TEMP-MISMATCH", span)],
            values: Vec::new(),
            store: Form::atom("NCL-SETF-STORE-MISMATCH", span),
            store_form: Form::atom("NCL-SETF-STORE-MISMATCH", span),
            access_form: Form::atom("NCL-SETF-ACCESS-MISMATCH", span),
        };

        let error = runtime
            .apply_setf_expansion(&expansion, Value::Integer(1), &environment, span)
            .map_or_else(
                |error| error,
                |value| panic!("mismatched temporary/value lists must be rejected, got {value:?}"),
            );

        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "SETF expansion temporary and value lists must have the same length"
        ));
    }
}
