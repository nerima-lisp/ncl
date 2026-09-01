use super::{Environment, Form, Runtime, RuntimeError, Span, Value};

impl Runtime {
    pub(super) fn set_property_place(
        &self,
        operator: &str,
        args: &[Form],
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        match operator {
            "SYMBOL-PLIST" => {
                if args.len() != 1 {
                    return Err(Self::arity("setf symbol-plist", "one", args.len()));
                }
                let symbol = self.eval_in(&args[0], environment)?;
                if symbol.symbol_reference().is_none() {
                    return Err(Self::invalid(
                        "setf symbol-plist target must be a symbol",
                        args[0].span,
                    ));
                }
                let Some(properties) = value.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                if !properties.len().is_multiple_of(2) {
                    return Err(Self::invalid(
                        "SYMBOL-PLIST needs an even property list",
                        args[0].span,
                    ));
                }
                environment.set_symbol_plist(&symbol, value);
                Ok(())
            }
            "GET" => {
                if args.len() != 2 {
                    return Err(Self::arity("setf get", "two", args.len()));
                }
                let symbol = self.eval_in(&args[0], environment)?;
                if symbol.symbol_reference().is_none() {
                    return Err(Self::invalid(
                        "setf get target must be a symbol",
                        args[0].span,
                    ));
                }
                let indicator = self.eval_in(&args[1], environment)?;
                let plist = environment.symbol_plist(&symbol).unwrap_or(Value::Nil);
                let Some(mut properties) = plist.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: plist.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                Self::replace_setf_property(
                    &mut properties,
                    indicator,
                    value,
                    "SETF GET",
                    args[0].span,
                )?;
                environment.set_symbol_plist(&symbol, Value::list(properties));
                Ok(())
            }
            "GETHASH" => {
                if args.len() != 2 {
                    return Err(Self::arity("setf gethash", "two", args.len()));
                }
                let key = self.eval_in(&args[0], environment)?;
                let table = self.eval_in(&args[1], environment)?;
                let Some(test) = table.hash_table_test() else {
                    return Err(RuntimeError::Type {
                        expected: "HASH-TABLE".to_string(),
                        actual: table.type_name().to_string(),
                        span: Some(args[1].span),
                    });
                };
                let test = test.to_string();
                let Some(entries) = table.hash_table_entries() else {
                    return Err(RuntimeError::Type {
                        expected: "HASH-TABLE".to_string(),
                        actual: table.type_name().to_string(),
                        span: Some(args[1].span),
                    });
                };
                let mut entries = entries.borrow_mut();
                if let Some((_, slot)) = entries.iter_mut().find(|(stored_key, _)| {
                    crate::builtins::hash_table_key_equal(&test, stored_key, &key)
                }) {
                    *slot = value;
                } else {
                    entries.push((key, value));
                }
                Ok(())
            }
            "GETF" => {
                if args.len() != 2 {
                    return Err(Self::arity("setf getf", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let indicator = self.eval_in(&args[1], environment)?;
                let Some(mut properties) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                Self::replace_setf_property(
                    &mut properties,
                    indicator,
                    value,
                    "GETF",
                    args[0].span,
                )?;
                self.set_place(&args[0], Value::list(properties), environment)
            }
            _ => unreachable!("set_property_place called with unsupported operator"),
        }
    }

    fn replace_setf_property(
        properties: &mut Vec<Value>,
        indicator: Value,
        value: Value,
        operation: &str,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if !properties.len().is_multiple_of(2) {
            let message = match operation {
                "SETF GET" => "SETF GET needs an even property list",
                "GETF" => "GETF needs an even property list",
                _ => "SETF property list must contain pairs",
            };
            return Err(Self::invalid(message, span));
        }
        if let Some(index) = (0..properties.len())
            .step_by(2)
            .find(|&index| properties[index].eq_value(&indicator))
            .map(|index| index + 1)
        {
            properties[index] = value;
        } else {
            properties.extend([indicator, value]);
        }
        Ok(())
    }
}
