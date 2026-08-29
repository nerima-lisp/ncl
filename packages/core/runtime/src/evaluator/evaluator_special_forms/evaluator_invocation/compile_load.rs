use super::{Compiler, Environment, Rc, Runtime, RuntimeError, Span, Value, fs};

impl Runtime {
    pub(crate) fn compile_function(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !(1..=2).contains(&arguments.len()) {
            return Err(Self::arity("compile", "one or two", arguments.len()));
        }

        let name = match &arguments[0] {
            Value::Nil | Value::Boolean(false) => None,
            value => {
                let (name, exact) = value
                    .symbol_reference()
                    .ok_or_else(|| Self::invalid("compile name must be a symbol or NIL", span))?;
                Some((name.to_owned(), exact))
            }
        };

        let function = match arguments.get(1) {
            None | Some(Value::Nil | Value::Boolean(false)) => {
                let Some((name, exact)) = name.as_ref() else {
                    return Err(Self::invalid(
                        "compile needs a function definition when the name is NIL",
                        span,
                    ));
                };
                let function = if *exact {
                    self.lookup_function_exact_in(name, environment)
                } else {
                    self.lookup_function_in(name, environment)
                };
                match function {
                    Some(value @ Value::Function(_)) => value,
                    Some(value) => {
                        return Err(RuntimeError::NotCallable {
                            value: value.to_string(),
                            span: Some(span),
                        });
                    }
                    None => {
                        return Err(RuntimeError::UnboundVariable {
                            name: name.clone(),
                            span: Some(span),
                        });
                    }
                }
            }
            Some(definition) => {
                let form = Self::form_from_value(definition, span)?;
                let expanded = self.prepare_compiled_form(&form, environment)?;
                let program = Rc::new(Compiler::compile_form(&expanded)?);
                crate::vm::run_entry(self, &program, 0, environment, expanded.span)?.primary_value()
            }
        };

        if !matches!(function, Value::Function(_)) {
            return Err(RuntimeError::Type {
                expected: "FUNCTION".to_owned(),
                actual: function.type_name().to_owned(),
                span: Some(span),
            });
        }

        if let Some((name, exact)) = name {
            if exact {
                environment.define_function_exact(name, function.clone());
            } else {
                environment.define_function(name, function.clone());
            }
        }

        Ok(Value::values(vec![function, Value::Nil, Value::Nil]))
    }

    pub(crate) fn load_file(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("load", "one", arguments.len()));
        }
        let path = match &arguments[0] {
            Value::String(path) => path.to_string(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "PATHNAME-DESIGNATOR".to_owned(),
                    actual: value.type_name().to_owned(),
                    span: Some(span),
                });
            }
        };
        let source = fs::read_to_string(&path)
            .map_err(|error| RuntimeError::Io(format!("cannot load {path}: {error}")))?;
        self.eval_source(&source)?;
        Ok(Value::boolean(true))
    }
}
