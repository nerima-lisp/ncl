impl Runtime {
    fn bind_macro_pattern(
        &self,
        pattern: &MacroPattern,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        match pattern {
            MacroPattern::Name(name) => {
                environment.define(name, value);
                Ok(())
            }
            MacroPattern::List(patterns) => {
                let Some(values) = value.list_items() else {
                    return Err(
                        self.invalid("macro destructuring pattern requires a proper list", span)
                    );
                };
                if values.len() != patterns.len() {
                    return Err(self.invalid(
                        "macro destructuring pattern has the wrong number of elements",
                        span,
                    ));
                }
                for (pattern, value) in patterns.iter().zip(values) {
                    self.bind_macro_pattern(pattern, value, environment, span)?;
                }
                Ok(())
            }
            MacroPattern::Dotted { items, tail } => {
                let Some((values, dotted_tail)) = macro_dotted_parts(&value) else {
                    return Err(self.invalid("macro destructuring pattern requires a list", span));
                };
                if values.len() < items.len() {
                    return Err(
                        self.invalid("macro destructuring pattern has too few elements", span)
                    );
                }
                for (pattern, value) in items.iter().zip(values.iter().cloned()) {
                    self.bind_macro_pattern(pattern, value, environment, span)?;
                }
                let remaining = values[items.len()..].to_vec();
                let tail_value = if remaining.is_empty() {
                    dotted_tail
                } else if dotted_tail.is_truthy() {
                    Value::dotted_list(remaining, dotted_tail)
                } else {
                    Value::list(remaining)
                };
                self.bind_macro_pattern(tail, tail_value, environment, span)
            }
            MacroPattern::LambdaList(lambda_list) => {
                self.bind_destructuring_lambda_list(lambda_list, value, environment, span)
            }
        }
    }

    fn bind_destructuring_lambda_list(
        &self,
        lambda_list: &MacroLambdaList,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if let Some(environment_name) = &lambda_list.environment {
            environment.define(environment_name, Value::environment(environment.clone()));
        }
        if let Some(whole) = &lambda_list.whole {
            environment.define(whole, value.clone());
        }
        let Some(arguments) = value.list_items() else {
            return Err(self.invalid("destructuring-bind value must be a proper list", span));
        };
        let required_count = lambda_list.required.len();
        let optional_count = lambda_list.optional.len();
        if arguments.len() < required_count {
            return Err(self.arity(
                "destructuring-bind",
                &format!("at least {required_count}"),
                arguments.len(),
            ));
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
            return Err(self.arity(
                "destructuring-bind",
                &format!("at most {maximum}"),
                arguments.len(),
            ));
        }

        for (pattern, argument) in lambda_list
            .required
            .iter()
            .zip(arguments.iter().take(required_count).cloned())
        {
            self.bind_macro_pattern(pattern, argument, environment, span)?;
        }
        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None => self.eval_in(&specification.init_form, environment)?,
            };
            self.bind_macro_pattern(&specification.pattern, value, environment, span)?;
            if let Some(supplied_p) = &specification.supplied_p {
                environment.define(supplied_p, Value::boolean(supplied.is_some()));
            }
        }
        if let Some(rest_name) = &lambda_list.rest {
            environment.define(rest_name, Value::list(arguments[key_start..].to_vec()));
        }

        if lambda_list.has_keyword_section {
            let keyword_arguments = &arguments[key_start..];
            if keyword_arguments.len() % 2 != 0 {
                return Err(self.invalid("keyword arguments must be supplied in pairs", span));
            }
            let mut supplied_keywords = HashMap::new();
            let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
            for pair in keyword_arguments.chunks_exact(2) {
                let keyword = match &pair[0] {
                    Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword,
                    _ => {
                        return Err(self.invalid("keyword argument name must be a keyword", span));
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
                            .any(|specification| specification.keyword_name == *keyword_name)
                    {
                        return Err(self.invalid(&format!("unknown keyword :{keyword_name}"), span));
                    }
                }
            }
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords.get(&specification.keyword_name);
                let value = match supplied {
                    Some(argument) => argument.clone(),
                    None => self.eval_in(&specification.init_form, environment)?,
                };
                self.bind_macro_pattern(&specification.pattern, value, environment, span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    environment.define(supplied_p, Value::boolean(supplied.is_some()));
                }
            }
        }
        for specification in &lambda_list.auxiliary {
            let value = self.eval_in(&specification.init_form, environment)?;
            environment.define(&specification.name, value);
        }
        Ok(())
    }


}
