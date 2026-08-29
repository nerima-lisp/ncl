use ncl_syntax::Span;

use crate::value::MacroLambdaList;
use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(in crate::evaluator) fn bind_destructuring_lambda_list(
        &self,
        lambda_list: &MacroLambdaList,
        value: &Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let Some(arguments) = value.list_items() else {
            return Err(Self::invalid(
                "destructuring-bind value must be a proper list",
                span,
            ));
        };
        let required_count = lambda_list.required.len();
        let optional_count = lambda_list.optional.len();
        if arguments.len() < required_count {
            return Err(Self::arity(
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
            return Err(Self::arity(
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
            Self::bind_macro_pattern(pattern, argument, environment, span)?;
        }
        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None => self.eval_in(&specification.init_form, environment)?,
            };
            Self::bind_macro_pattern(&specification.pattern, value, environment, span)?;
            if let Some(supplied_p) = &specification.supplied_p {
                environment.define(supplied_p, Value::boolean(supplied.is_some()));
            }
        }
        if let Some(rest_name) = &lambda_list.rest {
            environment.define(rest_name, Value::list(arguments[key_start..].to_vec()));
        }

        if let Some(supplied_keywords) = lambda_list
            .has_keyword_section
            .then(|| Self::parse_destructuring_keywords(&arguments[key_start..], lambda_list, span))
            .transpose()?
        {
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords.get(&specification.keyword_name);
                let value = match supplied {
                    Some(argument) => argument.clone(),
                    None => self.eval_in(&specification.init_form, environment)?,
                };
                Self::bind_macro_pattern(&specification.pattern, value, environment, span)?;
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

#[cfg(test)]
mod tests {
    use crate::Runtime;

    fn last_result_string(runtime: &Runtime, source: &str) -> String {
        let values = runtime
            .eval_source(source)
            .unwrap_or_else(|error| panic!("expected {source} to evaluate: {error}"));
        values
            .last()
            .unwrap_or_else(|| panic!("expected {source} to produce a value"))
            .to_string()
    }

    #[test]
    fn an_unsupplied_optional_parameter_evaluates_its_init_form() {
        let runtime = Runtime::new();
        assert_eq!(
            last_result_string(
                &runtime,
                "(destructuring-bind (&optional (a (+ 1 2))) nil a)"
            ),
            "3"
        );
    }

    #[test]
    fn a_non_keyword_key_argument_name_is_rejected() {
        let runtime = Runtime::new();
        assert!(
            runtime
                .eval_source("(destructuring-bind (&key a) (list 1 2) a)")
                .is_err()
        );
    }
}
