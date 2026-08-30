#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn special_funcall(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity("funcall", "at least one", 0));
        }
        let function = self.eval_in(&items[1], environment)?;
        let arguments = items[2..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_in(&function, &arguments, items[0].span, environment)
    }

    pub(crate) fn special_eval(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(Self::arity("eval", "one", items.len().saturating_sub(1)));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = Self::form_from_value(&value, items[1].span)?;
        self.eval_values_in(&form, environment)
    }

    pub(crate) fn special_apply(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "apply",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        let function = self.eval_in(&items[1], environment)?;
        let evaluated = items[2..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(last) = evaluated.last() else {
            return Err(Self::invalid("apply needs a final list", items[0].span));
        };
        let Some(mut final_arguments) = last.list_items() else {
            return Err(Self::invalid(
                "apply's final argument must be a list",
                items[0].span,
            ));
        };
        let mut arguments = evaluated[..evaluated.len() - 1].to_vec();
        arguments.append(&mut final_arguments);
        self.apply_in(&function, &arguments, items[0].span, environment)
    }

    pub(crate) fn resolve_function_designator(
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
                    crate::environment::intern_name(name).to_string()
                },
                span: Some(span),
            }),
        }
    }
}
