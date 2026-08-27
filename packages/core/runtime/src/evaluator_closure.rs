use super::{
    ClosureApplicationContext, ClosureKeywordApplicationContext, Environment, HashMap,
    LambdaListAuxiliaryParameter, Runtime, RuntimeError, Value,
};

impl Runtime {
    pub(super) fn apply_closure(
        &self,
        context: &ClosureApplicationContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let ClosureApplicationContext {
            parameters,
            required_escaped,
            optional,
            rest,
            rest_escaped,
            keywords,
            has_keyword_section,
            allow_other_keys,
            auxiliary,
            body,
            environment,
            arguments,
            span,
        } = *context;
        let required_count = parameters.len();
        let optional_count = optional.len();
        let maximum_count = required_count + optional_count;
        if arguments.len() < required_count {
            let expected = if optional_count > 0 || rest.is_some() || has_keyword_section {
                format!("at least {required_count}")
            } else {
                required_count.to_string()
            };
            return Err(Self::arity("closure", &expected, arguments.len()));
        }
        let optional_supplied_count = if has_keyword_section {
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
        if !has_keyword_section && rest.is_none() && arguments.len() > maximum_count {
            let expected = if optional_count > 0 {
                format!("at most {maximum_count}")
            } else {
                maximum_count.to_string()
            };
            return Err(Self::arity("closure", &expected, arguments.len()));
        }

        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        for (index, (parameter, argument)) in parameters.iter().zip(arguments.iter()).enumerate() {
            if required_escaped.get(index).copied().unwrap_or(false) {
                self.define_exact_in(parameter, argument.clone(), &local);
            } else {
                self.define_in(parameter, argument.clone(), &local);
            }
        }
        for (index, specification) in optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None => self.eval_in(&specification.init_form, &local)?,
            };
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value, &local);
            } else {
                self.define_in(&specification.name, value, &local);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                let supplied_value = Value::boolean(supplied.is_some());
                if specification.supplied_p_escaped.unwrap_or(false) {
                    self.define_exact_in(supplied_p, supplied_value, &local);
                } else {
                    self.define_in(supplied_p, supplied_value, &local);
                }
            }
        }
        self.apply_closure_rest(rest, rest_escaped, &arguments[key_start..], &local);
        if has_keyword_section {
            self.apply_closure_keywords(&ClosureKeywordApplicationContext {
                keywords,
                arguments,
                key_start,
                allow_other_keys,
                local: &local,
                span,
            })?;
        }
        self.apply_closure_auxiliary(auxiliary, &local)?;
        self.eval_sequence_values(body, &local)
    }

    fn apply_closure_rest(
        &self,
        rest: Option<&String>,
        rest_escaped: bool,
        arguments: &[Value],
        local: &Environment,
    ) {
        if let Some(rest) = rest {
            let value = Value::list(arguments.to_vec());
            if rest_escaped {
                self.define_exact_in(rest, value, local);
            } else {
                self.define_in(rest, value, local);
            }
        }
    }

    fn apply_closure_auxiliary(
        &self,
        auxiliary: &[LambdaListAuxiliaryParameter],
        local: &Environment,
    ) -> Result<(), RuntimeError> {
        for specification in auxiliary {
            let value = self.eval_in(&specification.init_form, local)?;
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value, local);
            } else {
                self.define_in(&specification.name, value, local);
            }
        }
        Ok(())
    }

    fn apply_closure_keywords(
        &self,
        context: &ClosureKeywordApplicationContext<'_>,
    ) -> Result<(), RuntimeError> {
        let ClosureKeywordApplicationContext {
            keywords,
            arguments,
            key_start,
            allow_other_keys,
            local,
            span,
        } = *context;
        let keyword_arguments = &arguments[key_start..];
        if !keyword_arguments.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "keyword arguments must be supplied in pairs",
                span,
            ));
        }
        let mut supplied_keywords = HashMap::new();
        let mut accepts_unknown_keywords = allow_other_keys;
        for pair in keyword_arguments.as_chunks::<2>().0 {
            let (Value::Keyword(keyword) | Value::KeywordExact(keyword)) = &pair[0] else {
                return Err(Self::invalid(
                    "keyword argument name must be a keyword",
                    span,
                ));
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
                    && !keywords
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
        for specification in keywords {
            let supplied = supplied_keywords.get(&specification.keyword_name);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None => self.eval_in(&specification.init_form, local)?,
            };
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value, local);
            } else {
                self.define_in(&specification.name, value, local);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                let supplied_value = Value::boolean(supplied.is_some());
                if specification.supplied_p_escaped.unwrap_or(false) {
                    self.define_exact_in(supplied_p, supplied_value, local);
                } else {
                    self.define_in(supplied_p, supplied_value, local);
                }
            }
        }
        Ok(())
    }
}
