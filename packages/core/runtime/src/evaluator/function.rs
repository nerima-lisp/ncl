use std::rc::Rc;

use ncl_syntax::Span;

use crate::environment::normalize_name;
use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(super) fn resolve_function_designator(
        &self,
        function: &Value,
        span: Span,
        environment: &Environment,
    ) -> Result<Rc<crate::Function>, RuntimeError> {
        if let Value::Function(function) = function {
            return Ok(function.clone());
        }

        let Some((name, exact)) = function.symbol_reference() else {
            return Err(RuntimeError::NotCallable {
                value: function.to_string(),
                span: Some(span),
            });
        };
        let resolved = if exact {
            self.lookup_function_exact_in(name, environment)
        } else {
            self.lookup_function_in(name, environment)
        };
        match resolved {
            Some(Value::Function(function)) => Ok(function),
            Some(value) => Err(RuntimeError::NotCallable {
                value: value.to_string(),
                span: Some(span),
            }),
            None => Err(RuntimeError::UnboundVariable {
                name: if exact {
                    name.to_string()
                } else {
                    normalize_name(name)
                },
                span: Some(span),
            }),
        }
    }
}
