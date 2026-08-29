#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_symbol_function_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        match name {
            "FBOUNDP" => Some(self.apply_fboundp(arguments, environment, span)),
            "MACRO-FUNCTION" => Some(self.apply_macro_function(arguments, span)),
            "SPECIAL-OPERATOR-P" => Some(Self::apply_special_operator_p(arguments, span)),
            "COMPILED-FUNCTION-P" => Some(Self::apply_compiled_function_p(arguments)),
            "FDEFINITION" | "SYMBOL-FUNCTION" => {
                Some(self.apply_function_definition(name, arguments, environment, span))
            }
            _ => None,
        }
    }

    fn apply_fboundp(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("fboundp", "one", arguments.len()));
        }
        let (name, exact) = arguments[0]
            .symbol_reference()
            .ok_or_else(|| Self::invalid("fboundp argument must be a symbol", span))?;
        let value = if exact {
            self.lookup_function_exact_in(name, environment)
        } else {
            self.lookup_function_in(name, environment)
        };
        Ok(Value::boolean(matches!(value, Some(Value::Function(_)))))
    }

    fn apply_macro_function(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 && arguments.len() != 2 {
            return Err(Self::arity("macro-function", "one or two", arguments.len()));
        }
        let (name, exact) = arguments[0]
            .symbol_reference()
            .ok_or_else(|| Self::invalid("macro-function argument must be a symbol", span))?;
        let environment = match arguments.get(1) {
            None | Some(Value::Nil | Value::Boolean(false)) => &self.global,
            Some(Value::Environment(environment)) => environment,
            Some(_) => {
                return Err(Self::invalid(
                    "macro-function environment must be an environment",
                    span,
                ));
            }
        };
        let value = if exact {
            self.lookup_function_exact_in(name, environment)
        } else {
            self.lookup_function_in(name, environment)
        };
        Ok(match value {
            Some(Value::Function(function))
                if matches!(
                    function.as_ref(),
                    crate::Function::Macro { .. } | crate::Function::ModifyMacro { .. }
                ) =>
            {
                Value::Function(function)
            }
            _ => Value::Nil,
        })
    }

    fn apply_special_operator_p(arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("special-operator-p", "one", arguments.len()));
        }
        let (name, _) = arguments[0]
            .symbol_reference()
            .ok_or_else(|| Self::invalid("special-operator-p argument must be a symbol", span))?;
        Ok(Value::boolean(is_special_operator_name(name)))
    }

    fn apply_compiled_function_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("compiled-function-p", "one", arguments.len()));
        }
        Ok(Value::boolean(matches!(
            &arguments[0],
            Value::Function(function) if matches!(function.as_ref(), crate::Function::Compiled { .. })
        )))
    }

    fn apply_function_definition(
        &self,
        primitive: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity(
                &primitive.to_ascii_lowercase(),
                "one",
                arguments.len(),
            ));
        }
        let (name, exact) = arguments[0]
            .symbol_reference()
            .ok_or_else(|| Self::invalid("function argument must be a symbol", span))?;
        let value = if exact {
            self.lookup_function_exact_in(name, environment)
        } else {
            self.lookup_function_in(name, environment)
        };
        match value {
            Some(Value::Function(function)) => Ok(Value::Function(function)),
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
