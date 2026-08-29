use super::{Environment, Form, Runtime, RuntimeError, Span, Value};
use crate::environment::normalize_name;

impl Runtime {
    pub(super) fn set_symbol_cell_place(
        &self,
        operator: &str,
        args: &[Form],
        value: Value,
        environment: &Environment,
        place_span: Span,
    ) -> Result<(), RuntimeError> {
        match operator {
            "SYMBOL-VALUE" => {
                if args.len() != 1 {
                    return Err(Self::arity("setf symbol-value", "one", args.len()));
                }
                let symbol = self.eval_in(&args[0], environment)?;
                let (name, exact) = symbol.symbol_reference().ok_or_else(|| {
                    Self::invalid("setf symbol-value target must be a symbol", args[0].span)
                })?;
                self.ensure_symbol_writable(name, exact, args[0].span)?;
                if exact {
                    self.set_symbol_value_exact(name, value);
                } else {
                    self.set_symbol_value(name, value);
                }
                Ok(())
            }
            "SYMBOL-FUNCTION" => {
                if args.len() != 1 {
                    return Err(Self::arity("setf symbol-function", "one", args.len()));
                }
                if !matches!(&value, Value::Function(_)) {
                    return Err(RuntimeError::Type {
                        expected: "FUNCTION".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place_span),
                    });
                }
                let symbol = self.eval_in(&args[0], environment)?;
                let (name, exact) = symbol.symbol_reference().ok_or_else(|| {
                    Self::invalid("setf symbol-function target must be a symbol", args[0].span)
                })?;
                if exact {
                    self.global.define_function_exact(name, value);
                } else {
                    let function_name = self
                        .dynamic_candidates(name)
                        .into_iter()
                        .next()
                        .unwrap_or_else(|| normalize_name(name));
                    self.global.define_function(function_name, value);
                }
                Ok(())
            }
            _ => unreachable!("set_symbol_cell_place called with unsupported operator"),
        }
    }
}
