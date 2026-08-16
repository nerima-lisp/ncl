pub(crate) fn run_entry(
    runtime: &Runtime,
    program: Rc<Program>,
    function_id: FunctionId,
    environment: Environment,
    span: Span,
) -> Result<Value, RuntimeError> {
    let Some(function) = program.functions.get(function_id) else {
        return Err(invalid("compiled function id is out of range", span));
    };
    if !function.parameters.is_empty()
        || !function.optional.is_empty()
        || !function.keywords.is_empty()
        || function.has_keyword_section
        || function.rest.is_some()
        || !function.auxiliary.is_empty()
    {
        return Err(RuntimeError::Arity {
            function: function
                .name
                .as_deref()
                .unwrap_or("compiled entry function")
                .to_string(),
            expected: "0".to_string(),
            actual: 0,
        });
    }
    run_code(runtime, &program, function, environment, span)
}

pub(crate) fn run(
    runtime: &Runtime,
    program: Rc<Program>,
    function_id: FunctionId,
    environment: Environment,
    arguments: &[Value],
    span: Span,
) -> Result<Value, RuntimeError> {
    let Some(function) = program.functions.get(function_id) else {
        return Err(invalid("compiled function id is out of range", span));
    };
    let required_count = function.parameters.len();
    let optional_count = function.optional.len();
    let maximum_count = required_count + optional_count;
    if arguments.len() < required_count {
        let expected =
            if optional_count > 0 || function.rest.is_some() || function.has_keyword_section {
                format!("at least {required_count}")
            } else {
                required_count.to_string()
            };
        return Err(RuntimeError::Arity {
            function: function
                .name
                .as_deref()
                .unwrap_or("compiled function")
                .to_string(),
            expected,
            actual: arguments.len(),
        });
    }
    let optional_supplied_count = if function.has_keyword_section {
        let available = arguments
            .len()
            .saturating_sub(required_count)
            .min(optional_count);
        (0..available)
            .take_while(|index| {
                !matches!(
                    arguments[required_count + *index],
                    Value::Keyword(_) | Value::KeywordExact(_)
                )
            })
            .count()
    } else {
        arguments
            .len()
            .saturating_sub(required_count)
            .min(optional_count)
    };
    let key_start = required_count + optional_supplied_count;
    if !function.has_keyword_section && function.rest.is_none() && arguments.len() > maximum_count {
        let expected = if optional_count > 0 {
            format!("at most {maximum_count}")
        } else {
            maximum_count.to_string()
        };
        return Err(RuntimeError::Arity {
            function: function
                .name
                .as_deref()
                .unwrap_or("compiled function")
                .to_string(),
            expected,
            actual: arguments.len(),
        });
    }

    let local = environment.child();
    let _dynamic_guard = runtime.dynamic_guard();
    for (index, (parameter, argument)) in
        function.parameters.iter().zip(arguments.iter()).enumerate()
    {
        if function
            .required_escaped
            .get(index)
            .copied()
            .unwrap_or(false)
        {
            runtime.define_exact_in(parameter, argument.clone(), &local);
        } else {
            runtime.define_in(parameter, argument.clone(), &local);
        }
    }
    for (index, specification) in function.optional.iter().enumerate() {
        let supplied =
            (index < optional_supplied_count).then(|| &arguments[required_count + index]);
        let value = if let Some(argument) = supplied {
            argument.clone()
        } else {
            let Some(default_function) = program.functions.get(specification.default_function)
            else {
                return Err(RuntimeError::InvalidForm {
                    message: "compiled optional default is out of range".to_string(),
                    span: Some(span),
                });
            };
            run_code(runtime, &program, default_function, local.clone(), span)?.primary_value()
        };
        if specification.name_escaped {
            runtime.define_exact_in(&specification.name, value, &local);
        } else {
            runtime.define_in(&specification.name, value, &local);
        }
        if let Some(supplied_p) = &specification.supplied_p {
            if specification.supplied_p_escaped.unwrap_or(false) {
                runtime.define_exact_in(supplied_p, Value::boolean(supplied.is_some()), &local);
            } else {
                runtime.define_in(supplied_p, Value::boolean(supplied.is_some()), &local);
            }
        }
    }
    if let Some(rest) = &function.rest {
        let rest_start = key_start;
        let value = Value::list(arguments[rest_start..].to_vec());
        if function.rest_escaped {
            runtime.define_exact_in(rest, value, &local);
        } else {
            runtime.define_in(rest, value, &local);
        }
    }
    if function.has_keyword_section {
        let keyword_arguments = &arguments[key_start..];
        if !keyword_arguments.len().is_multiple_of(2) {
            return Err(RuntimeError::InvalidForm {
                message: "keyword arguments must be supplied in pairs".to_string(),
                span: Some(span),
            });
        }
        let mut supplied_keywords = HashMap::new();
        let mut accepts_unknown_keywords = function.allow_other_keys;
        for pair in keyword_arguments.chunks_exact(2) {
            let keyword = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword,
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: "keyword argument name must be a keyword".to_string(),
                        span: Some(span),
                    });
                }
            };
            let keyword_name = keyword.to_string();
            if keyword_name == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
                accepts_unknown_keywords = true;
            }
            supplied_keywords.insert(keyword_name, pair[1].clone());
        }
        if !accepts_unknown_keywords {
            for keyword_name in supplied_keywords.keys() {
                if keyword_name != "ALLOW-OTHER-KEYS"
                    && !function
                        .keywords
                        .iter()
                        .any(|specification| specification.keyword_name == *keyword_name)
                {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }
        for specification in &function.keywords {
            let supplied = supplied_keywords.get(&specification.keyword_name);
            let value = if let Some(argument) = supplied {
                argument.clone()
            } else {
                let Some(default_function) = program.functions.get(specification.default_function)
                else {
                    return Err(RuntimeError::InvalidForm {
                        message: "compiled keyword default is out of range".to_string(),
                        span: Some(span),
                    });
                };
                run_code(runtime, &program, default_function, local.clone(), span)?.primary_value()
            };
            if specification.name_escaped {
                runtime.define_exact_in(&specification.name, value, &local);
            } else {
                runtime.define_in(&specification.name, value, &local);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                if specification.supplied_p_escaped.unwrap_or(false) {
                    runtime.define_exact_in(supplied_p, Value::boolean(supplied.is_some()), &local);
                } else {
                    runtime.define_in(supplied_p, Value::boolean(supplied.is_some()), &local);
                }
            }
        }
    }
    for specification in &function.auxiliary {
        let Some(default_function) = program.functions.get(specification.default_function) else {
            return Err(RuntimeError::InvalidForm {
                message: "compiled auxiliary default is out of range".to_string(),
                span: Some(span),
            });
        };
        let value =
            run_code(runtime, &program, default_function, local.clone(), span)?.primary_value();
        if specification.name_escaped {
            runtime.define_exact_in(&specification.name, value, &local);
        } else {
            runtime.define_in(&specification.name, value, &local);
        }
    }
    run_code(runtime, &program, function, local, span)
}
