impl Runtime {
    fn set_symbol_place(
        &self,
        operator: &str,
        args: &[Form],
        value: &Value,
        place: &Form,
        environment: &Environment,
    ) -> Option<Result<(), RuntimeError>> {
        Some(match operator {
            "SYMBOL-VALUE" => {
                if args.len() != 1 {
                    return Some(Err(self.arity("setf symbol-value", "one", args.len())));
                }
                let symbol = match self.eval_in(&args[0], environment) {
                    Ok(symbol) => symbol,
                    Err(error) => return Some(Err(error)),
                };
                let (name, exact) = match symbol.symbol_reference() {
                    Some(reference) => reference,
                    None => {
                        return Some(Err(self.invalid(
                            "setf symbol-value target must be a symbol",
                            args[0].span,
                        )));
                    }
                };
                if let Err(error) = self.ensure_symbol_writable(name, exact, args[0].span) {
                    return Some(Err(error));
                }
                if exact {
                    self.set_symbol_value_exact(name, value.clone());
                } else {
                    self.set_symbol_value(name, value.clone());
                }
                Ok(())
            }
            "SYMBOL-FUNCTION" => {
                if args.len() != 1 {
                    return Some(Err(self.arity("setf symbol-function", "one", args.len())));
                }
                if !matches!(value, Value::Function(_)) {
                    return Some(Err(RuntimeError::Type {
                        expected: "FUNCTION".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    }));
                }
                let symbol = match self.eval_in(&args[0], environment) {
                    Ok(symbol) => symbol,
                    Err(error) => return Some(Err(error)),
                };
                let (name, exact) = match symbol.symbol_reference() {
                    Some(reference) => reference,
                    None => {
                        return Some(Err(self.invalid(
                            "setf symbol-function target must be a symbol",
                            args[0].span,
                        )));
                    }
                };
                if exact {
                    self.global.define_function_exact(name, value.clone());
                } else {
                    let function_name = self
                        .dynamic_candidates(name)
                        .into_iter()
                        .next()
                        .unwrap_or_else(|| normalize_name(name));
                    self.global.define_function(function_name, value.clone());
                }
                Ok(())
            }
            "MACRO-FUNCTION" => {
                if args.len() != 1 {
                    return Some(Err(self.arity("setf macro-function", "one", args.len())));
                }
                let symbol = match self.eval_in(&args[0], environment) {
                    Ok(symbol) => symbol,
                    Err(error) => return Some(Err(error)),
                };
                let (name, exact) = match symbol.symbol_reference() {
                    Some(reference) => reference,
                    None => {
                        return Some(Err(self.invalid(
                            "setf macro-function target must be a symbol",
                            args[0].span,
                        )));
                    }
                };
                match value {
                    Value::Nil => {
                        if exact {
                            self.global.remove_exact(name);
                        } else {
                            let function_name = self
                                .dynamic_candidates(name)
                                .into_iter()
                                .next()
                                .unwrap_or_else(|| normalize_name(name));
                            self.global.remove(&function_name);
                        }
                        Ok(())
                    }
                    Value::Function(function)
                        if matches!(
                            function.as_ref(),
                            crate::Function::Macro { .. } | crate::Function::ModifyMacro { .. }
                        ) =>
                    {
                        if exact {
                            self.global.define_exact(name, value.clone());
                        } else {
                            let function_name = self
                                .dynamic_candidates(name)
                                .into_iter()
                                .next()
                                .unwrap_or_else(|| normalize_name(name));
                            self.global.define(function_name, value.clone());
                        }
                        Ok(())
                    }
                    other => Err(RuntimeError::Type {
                        expected: "MACRO-FUNCTION or NIL".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(place.span),
                    }),
                }
            }
            "COMPILER-MACRO-FUNCTION" => {
                if args.len() != 1 {
                    return Some(Err(self.arity(
                        "setf compiler-macro-function",
                        "one",
                        args.len(),
                    )));
                }
                let symbol = match self.eval_in(&args[0], environment) {
                    Ok(symbol) => symbol,
                    Err(error) => return Some(Err(error)),
                };
                let (name, exact) = match symbol.symbol_reference() {
                    Some(reference) => reference,
                    None => {
                        return Some(Err(self.invalid(
                            "setf compiler-macro-function target must be a symbol",
                            args[0].span,
                        )));
                    }
                };
                match value {
                    Value::Nil => {
                        if exact {
                            self.global.remove_compiler_macro_exact(name);
                        } else {
                            let function_name = self
                                .dynamic_candidates(name)
                                .into_iter()
                                .next()
                                .unwrap_or_else(|| normalize_name(name));
                            self.global.remove_compiler_macro(&function_name);
                        }
                        Ok(())
                    }
                    Value::Function(function)
                        if matches!(
                            function.as_ref(),
                            crate::Function::Macro { .. } | crate::Function::ModifyMacro { .. }
                        ) =>
                    {
                        if exact {
                            self.global
                                .define_compiler_macro_exact(name, value.clone());
                        } else {
                            let function_name = self
                                .dynamic_candidates(name)
                                .into_iter()
                                .next()
                                .unwrap_or_else(|| normalize_name(name));
                            self.global
                                .define_compiler_macro(function_name, value.clone());
                        }
                        Ok(())
                    }
                    other => Err(RuntimeError::Type {
                        expected: "COMPILER-MACRO-FUNCTION or NIL".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(place.span),
                    }),
                }
            }
            "SYMBOL-PLIST" => {
                if args.len() != 1 {
                    return Some(Err(self.arity("setf symbol-plist", "one", args.len())));
                }
                let symbol = match self.eval_in(&args[0], environment) {
                    Ok(symbol) => symbol,
                    Err(error) => return Some(Err(error)),
                };
                if symbol.symbol_reference().is_none() {
                    return Some(Err(self.invalid(
                        "setf symbol-plist target must be a symbol",
                        args[0].span,
                    )));
                }
                if !matches!(value, Value::Nil | Value::List(_)) {
                    return Some(Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    }));
                }
                environment.set_symbol_plist(&symbol, value.clone());
                Ok(())
            }
            "GET" => {
                if args.len() != 2 {
                    return Some(Err(self.arity("setf get", "two", args.len())));
                }
                let symbol = match self.eval_in(&args[0], environment) {
                    Ok(symbol) => symbol,
                    Err(error) => return Some(Err(error)),
                };
                if symbol.symbol_reference().is_none() {
                    return Some(Err(self.invalid("setf get target must be a symbol", args[0].span)));
                }
                let indicator = match self.eval_in(&args[1], environment) {
                    Ok(indicator) => indicator,
                    Err(error) => return Some(Err(error)),
                };
                let plist = environment.symbol_plist(&symbol).unwrap_or(Value::Nil);
                let Some(mut properties) = plist.list_items() else {
                    return Some(Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: plist.type_name().to_string(),
                        span: Some(args[0].span),
                    }));
                };
                if properties.len() % 2 != 0 {
                    return Some(Err(self.invalid(
                        "SETF GET needs an even property list",
                        args[0].span,
                    )));
                }
                let mut found = None;
                for index in (0..properties.len()).step_by(2) {
                    if properties[index].eq_value(&indicator) {
                        found = Some(index + 1);
                        break;
                    }
                }
                if let Some(index) = found {
                    properties[index] = value.clone();
                } else {
                    properties.push(indicator);
                    properties.push(value.clone());
                }
                environment.set_symbol_plist(&symbol, Value::list(properties));
                Ok(())
            }
            "GETHASH" => {
                if args.len() != 2 {
                    return Some(Err(self.arity("setf gethash", "two", args.len())));
                }
                let key = match self.eval_in(&args[0], environment) {
                    Ok(key) => key,
                    Err(error) => return Some(Err(error)),
                };
                let table = match self.eval_in(&args[1], environment) {
                    Ok(table) => table,
                    Err(error) => return Some(Err(error)),
                };
                let Some(test) = table.hash_table_test() else {
                    return Some(Err(RuntimeError::Type {
                        expected: "HASH-TABLE".to_string(),
                        actual: table.type_name().to_string(),
                        span: Some(args[1].span),
                    }));
                };
                let test = test.to_string();
                let Some(entries) = table.hash_table_entries() else {
                    return Some(Err(RuntimeError::Type {
                        expected: "HASH-TABLE".to_string(),
                        actual: table.type_name().to_string(),
                        span: Some(args[1].span),
                    }));
                };
                let mut entries = entries.borrow_mut();
                if let Some((_, slot)) = entries.iter_mut().find(|(stored_key, _)| {
                    crate::builtins::hash_table_key_equal(&test, stored_key, &key)
                }) {
                    *slot = value.clone();
                } else {
                    entries.push((key, value.clone()));
                }
                Ok(())
            }
            "GETF" => {
                if args.len() != 2 {
                    return Some(Err(self.arity("setf getf", "two", args.len())));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                let indicator = match self.eval_in(&args[1], environment) {
                    Ok(indicator) => indicator,
                    Err(error) => return Some(Err(error)),
                };
                let Some(mut properties) = current.list_items() else {
                    return Some(Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    }));
                };
                if properties.len() % 2 != 0 {
                    return Some(Err(self.invalid("GETF needs an even property list", args[0].span)));
                }
                let mut found = None;
                for index in (0..properties.len()).step_by(2) {
                    if properties[index].eq_value(&indicator) {
                        found = Some(index + 1);
                        break;
                    }
                }
                if let Some(index) = found {
                    properties[index] = value.clone();
                } else {
                    properties.push(indicator);
                    properties.push(value.clone());
                }
                self.set_place(&args[0], Value::list(properties), environment)
            }
            "VALUES" => {
                let values = value.multiple_values();
                for (index, target) in args.iter().enumerate() {
                    if let Err(error) = self.set_place(
                        target,
                        values.get(index).cloned().unwrap_or(Value::Nil),
                        environment,
                    ) {
                        return Some(Err(error));
                    }
                }
                Ok(())
            }
            _ => return None,
        })
    }
}
