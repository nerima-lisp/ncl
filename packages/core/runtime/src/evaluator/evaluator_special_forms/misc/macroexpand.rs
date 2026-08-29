use ncl_syntax::{Form, Span};

use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_macroexpand(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity(
                "macroexpand",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = Self::form_from_value(&value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let (expanded, expanded_p) = self.expand_macros_with_flag(form, &expansion_environment)?;
        Ok(Value::values(vec![
            Self::quoted_value(&expanded)?,
            Value::boolean(expanded_p),
        ]))
    }

    pub(in crate::evaluator::evaluator_special_forms) fn macroexpansion_environment(
        &self,
        value: Value,
        span: Span,
    ) -> Result<Environment, RuntimeError> {
        match value {
            Value::Nil | Value::Boolean(false) => Ok(self.global.clone()),
            Value::Environment(environment) => Ok(environment),
            _ => Err(Self::invalid(
                "macro expansion environment must be an environment",
                span,
            )),
        }
    }
}
