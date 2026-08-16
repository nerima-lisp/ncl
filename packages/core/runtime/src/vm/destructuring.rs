
fn destructure_specification(
    specification: &DestructureSpec,
    value: Value,
    runtime: &Runtime,
    program: &Rc<Program>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    match specification {
        DestructureSpec::Pattern(pattern) => {
            destructure_value(pattern, value, runtime, program, environment, span)
        }
        DestructureSpec::LambdaList(lambda_list) => {
            destructure_lambda_list(lambda_list, value, runtime, program, environment, span)
        }
    }
}

fn destructure_lambda_list(
    lambda_list: &DestructureLambdaList,
    value: Value,
    runtime: &Runtime,
    program: &Rc<Program>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let Some(arguments) = value.list_items() else {
        return Err(invalid(
            "destructuring-bind value must be a proper list",
            span,
        ));
    };
    if let Some(environment_name) = &lambda_list.environment {
        runtime.define_in(
            environment_name,
            Value::environment(environment.clone()),
            environment,
        );
    }
    if let Some(whole) = &lambda_list.whole {
        runtime.define_in(whole, value.clone(), environment);
    }
    let required_count = lambda_list.required.len();
    let optional_count = lambda_list.optional.len();
    if arguments.len() < required_count {
        return Err(RuntimeError::Arity {
            function: "destructuring-bind".to_string(),
            expected: format!("at least {required_count}"),
            actual: arguments.len(),
        });
    }
    let optional_supplied_count = if lambda_list.has_keyword_section {
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
    if !lambda_list.has_keyword_section
        && lambda_list.rest.is_none()
        && arguments.len() > required_count + optional_count
    {
        let maximum = required_count + optional_count;
        return Err(RuntimeError::Arity {
            function: "destructuring-bind".to_string(),
            expected: format!("at most {maximum}"),
            actual: arguments.len(),
        });
    }

    for (pattern, argument) in lambda_list
        .required
        .iter()
        .zip(arguments.iter().take(required_count).cloned())
    {
        destructure_value(pattern, argument, runtime, program, environment, span)?;
    }
    for (index, parameter) in lambda_list.optional.iter().enumerate() {
        let supplied =
            (index < optional_supplied_count).then(|| arguments[required_count + index].clone());
        let value = if let Some(argument) = supplied.as_ref() {
            argument.clone()
        } else {
            let default_function = program
                .functions
                .get(parameter.default_function)
                .ok_or_else(|| {
                    invalid(
                        "compiled destructuring optional default is out of range",
                        span,
                    )
                })?;
            run_code(
                runtime,
                program,
                default_function,
                environment.clone(),
                span,
            )?
            .primary_value()
        };
        destructure_value(
            &parameter.pattern,
            value,
            runtime,
            program,
            environment,
            span,
        )?;
        if let Some(supplied_p) = &parameter.supplied_p {
            runtime.define_in(supplied_p, Value::boolean(supplied.is_some()), environment);
        }
    }
    if let Some(rest_name) = &lambda_list.rest {
        runtime.define_in(
            rest_name,
            Value::list(arguments[key_start..].to_vec()),
            environment,
        );
    }

    if lambda_list.has_keyword_section {
        let keyword_arguments = &arguments[key_start..];
        if keyword_arguments.len() % 2 != 0 {
            return Err(invalid("keyword arguments must be supplied in pairs", span));
        }
        let mut supplied_keywords = HashMap::new();
        let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
        for pair in keyword_arguments.chunks_exact(2) {
            let keyword = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword,
                _ => {
                    return Err(invalid("keyword argument name must be a keyword", span));
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
                    && !lambda_list
                        .keywords
                        .iter()
                        .any(|parameter| parameter.keyword_name == *keyword_name)
                {
                    return Err(invalid(&format!("unknown keyword :{keyword_name}"), span));
                }
            }
        }
        for parameter in &lambda_list.keywords {
            let supplied = supplied_keywords.get(&parameter.keyword_name);
            let value = if let Some(argument) = supplied {
                argument.clone()
            } else {
                let default_function = program
                    .functions
                    .get(parameter.default_function)
                    .ok_or_else(|| {
                        invalid(
                            "compiled destructuring keyword default is out of range",
                            span,
                        )
                    })?;
                run_code(
                    runtime,
                    program,
                    default_function,
                    environment.clone(),
                    span,
                )?
                .primary_value()
            };
            destructure_value(
                &parameter.pattern,
                value,
                runtime,
                program,
                environment,
                span,
            )?;
            if let Some(supplied_p) = &parameter.supplied_p {
                runtime.define_in(supplied_p, Value::boolean(supplied.is_some()), environment);
            }
        }
    }
    for parameter in &lambda_list.auxiliary {
        let default_function = program
            .functions
            .get(parameter.default_function)
            .ok_or_else(|| {
                invalid(
                    "compiled destructuring auxiliary default is out of range",
                    span,
                )
            })?;
        let value = run_code(
            runtime,
            program,
            default_function,
            environment.clone(),
            span,
        )?
        .primary_value();
        runtime.define_in(&parameter.name, value, environment);
    }
    Ok(())
}


fn destructure_value(
    pattern: &DestructurePattern,
    value: Value,
    runtime: &Runtime,
    program: &Rc<Program>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    match pattern {
        DestructurePattern::Name(name) => {
            runtime.define_in(name, value, environment);
            Ok(())
        }
        DestructurePattern::List(patterns) => {
            let Some(values) = value.list_items() else {
                return Err(invalid(
                    "destructuring-bind pattern requires a proper list",
                    span,
                ));
            };
            if values.len() != patterns.len() {
                return Err(invalid(
                    "destructuring-bind pattern has the wrong number of elements",
                    span,
                ));
            }
            for (pattern, value) in patterns.iter().zip(values) {
                destructure_value(pattern, value, runtime, program, environment, span)?;
            }
            Ok(())
        }
        DestructurePattern::LambdaList(lambda_list) => {
            destructure_lambda_list(lambda_list, value, runtime, program, environment, span)
        }
        DestructurePattern::Dotted { items, tail } => {
            let Some((values, dotted_tail)) = destructure_dotted_parts(&value) else {
                return Err(invalid("destructuring-bind pattern requires a list", span));
            };
            if values.len() < items.len() {
                return Err(invalid(
                    "destructuring-bind pattern has too few elements",
                    span,
                ));
            }
            for (pattern, value) in items.iter().zip(values.iter().cloned()) {
                destructure_value(pattern, value, runtime, program, environment, span)?;
            }
            let remaining = values[items.len()..].to_vec();
            let tail_value = if remaining.is_empty() {
                dotted_tail
            } else if dotted_tail.is_truthy() {
                Value::dotted_list(remaining, dotted_tail)
            } else {
                Value::list(remaining)
            };
            destructure_value(tail, tail_value, runtime, program, environment, span)
        }
    }
}


fn destructure_dotted_parts(value: &Value) -> Option<(Vec<Value>, Value)> {
    match value {
        Value::Nil => Some((Vec::new(), Value::Nil)),
        Value::List(values) => Some((values.as_ref().clone(), Value::Nil)),
        Value::DottedList { items, tail } => {
            let mut values = items.as_ref().clone();
            match tail.as_ref() {
                Value::Nil => Some((values, Value::Nil)),
                Value::List(more) => {
                    values.extend(more.iter().cloned());
                    Some((values, Value::Nil))
                }
                Value::DottedList { .. } => {
                    let (more, dotted_tail) = destructure_dotted_parts(tail)?;
                    values.extend(more);
                    Some((values, dotted_tail))
                }
                other => Some((values, other.clone())),
            }
        }
        _ => None,
    }
}
