#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_symbol_value_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "BOUNDP" | "CONSTANTP" | "SYMBOL-VALUE") {
            return None;
        }
        let result = (|| -> Result<Value, RuntimeError> {
            match name {
                "BOUNDP" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("boundp", "one", arguments.len()));
                    }
                    let (name, exact) = arguments[0]
                        .symbol_reference()
                        .ok_or_else(|| Self::invalid("boundp argument must be a symbol", span))?;
                    Ok(Value::boolean(if exact {
                        self.is_bound_exact_in(name, environment)
                    } else {
                        self.is_bound_in(name, environment)
                    }))
                }
                "CONSTANTP" => {
                    if arguments.len() != 1 && arguments.len() != 2 {
                        return Err(Self::arity("constantp", "one or two", arguments.len()));
                    }
                    Ok(Value::boolean(self.constantp(&arguments[0])))
                }
                "SYMBOL-VALUE" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("symbol-value", "one", arguments.len()));
                    }
                    let (name, exact) = arguments[0].symbol_reference().ok_or_else(|| {
                        Self::invalid("symbol-value argument must be a symbol", span)
                    })?;
                    let value = if exact {
                        self.lookup_exact_in(name, environment)
                    } else {
                        self.lookup_in(name, environment)
                    };
                    value.ok_or_else(|| RuntimeError::UnboundVariable {
                        name: if exact {
                            name.to_string()
                        } else {
                            normalize_name(name)
                        },
                        span: Some(span),
                    })
                }
                _ => unreachable!("symbol value name was prevalidated"),
            }
        })();
        Some(result)
    }
}
