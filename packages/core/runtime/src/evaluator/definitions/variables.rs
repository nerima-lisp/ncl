impl Runtime {
    fn setf_index(&self, value: Value, span: Span) -> Result<usize, RuntimeError> {
        match value {
            Value::Integer(index) if index >= 0 => {
                usize::try_from(index).map_err(|_| self.invalid("SETF index is too large", span))
            }
            Value::Integer(_) => Err(self.invalid("SETF index must be non-negative", span)),
            other => Err(RuntimeError::Type {
                expected: "INTEGER".to_string(),
                actual: other.type_name().to_string(),
                span: Some(span),
            }),
        }
    }

    fn special_defvar(
        &self,
        items: &[Form],
        environment: &Environment,
        force: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if force { "defparameter" } else { "defvar" };
        if !(2..=4).contains(&items.len()) {
            return Err(self.arity(operator, "one to three", items.len().saturating_sub(1)));
        }
        let context = if force {
            "defparameter name must be a symbol"
        } else {
            "defvar name must be a symbol"
        };
        let (name, escaped) = self.variable_name_info(&items[1], context)?;
        let documentation = match items.get(3) {
            Some(Form {
                kind: FormKind::String(documentation),
                ..
            }) => Some(documentation.clone()),
            Some(form) => {
                return Err(self.invalid("defvar documentation must be a string", form.span));
            }
            None => None,
        };
        if force
            && if escaped {
                self.is_constant_exact_in(&name)
            } else {
                self.is_constant_in(&name)
            }
        {
            return Err(self.constant_modification_error(&name, items[1].span));
        }
        if !force {
            let existing = if escaped {
                self.lookup_special_exact(&name)
            } else {
                self.lookup_special(&name)
            };
            if let Some(value) = existing {
                if let Some(documentation) = documentation {
                    if escaped {
                        environment.define_variable_documentation_exact(&name, documentation);
                    } else {
                        environment.define_variable_documentation(&name, documentation);
                    }
                }
                return Ok(value);
            }
        };
        let value = items
            .get(2)
            .map_or(Ok(Value::Nil), |form| self.eval_in(form, environment))?;
        let value = if escaped {
            self.define_special_value_exact(&name, value, force)
        } else {
            self.define_special_value(&name, value, force)
        };
        if let Some(documentation) = documentation {
            if escaped {
                environment.define_variable_documentation_exact(&name, documentation);
            } else {
                environment.define_variable_documentation(&name, documentation);
            }
        }
        Ok(value)
    }

    fn special_defconstant(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(self.arity("defconstant", "two or three", items.len().saturating_sub(1)));
        }
        let (name, escaped) =
            self.variable_name_info(&items[1], "defconstant name must be a symbol")?;
        if if escaped {
            self.is_constant_exact_in(&name)
        } else {
            self.is_constant_in(&name)
        } {
            return Err(self.constant_modification_error(&name, items[1].span));
        }
        let value = self.eval_in(&items[2], environment)?;
        Ok(if escaped {
            self.define_constant_value_exact(&name, value)
        } else {
            self.define_constant_value(&name, value)
        })
    }

}
