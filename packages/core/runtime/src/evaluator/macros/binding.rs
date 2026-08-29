use ncl_syntax::Form;

use crate::environment::normalize_name;
use crate::evaluator::MacroBindingContext;
use crate::evaluator::helpers::is_macro_keyword_form;
use crate::value::MacroAuxiliaryParameter;
use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(in crate::evaluator) fn invoke_macro(
        &self,
        context: MacroBindingContext<'_>,
        body: &[Form],
    ) -> Result<Value, RuntimeError> {
        let local = self.bind_macro_arguments(context)?;
        self.eval_sequence_values(body, &local)
    }

    pub(super) fn bind_macro_arguments(
        &self,
        context: MacroBindingContext<'_>,
    ) -> Result<Environment, RuntimeError> {
        let MacroBindingContext {
            form,
            arguments,
            macro_name,
            lambda_list,
            macro_environment,
            environment,
        } = context;
        let argument_count = arguments.len();
        let required_count = lambda_list.required.len();
        if argument_count < required_count {
            return Err(Self::arity(
                &normalize_name(macro_name),
                &format!("at least {required_count}"),
                argument_count,
            ));
        }

        let optional_supplied_count = if lambda_list.has_keyword_section {
            let available = argument_count
                .saturating_sub(required_count)
                .min(lambda_list.optional.len());
            (0..available)
                .take_while(|index| !is_macro_keyword_form(&arguments[index + required_count]))
                .count()
        } else {
            argument_count
                .saturating_sub(required_count)
                .min(lambda_list.optional.len())
        };
        let key_start = required_count + optional_supplied_count;
        if !lambda_list.has_keyword_section
            && lambda_list.rest.is_none()
            && argument_count > required_count + lambda_list.optional.len()
        {
            let maximum = required_count + lambda_list.optional.len();
            return Err(Self::arity(
                &normalize_name(macro_name),
                &format!("at most {maximum}"),
                argument_count,
            ));
        }

        let keyword_arguments = lambda_list
            .has_keyword_section
            .then(|| Self::parse_macro_keywords(&arguments[key_start..], lambda_list, form.span))
            .transpose()?;

        let local = macro_environment.child();
        if let Some(environment_name) = &lambda_list.environment {
            local.define(environment_name, Value::environment((*environment).clone()));
        }
        if let Some(whole) = &lambda_list.whole {
            local.define(whole, Self::quoted_value(form)?);
        }
        for (pattern, argument) in lambda_list
            .required
            .iter()
            .zip(arguments[..required_count].iter())
        {
            Self::bind_macro_pattern(
                pattern,
                Self::quoted_value(argument)?,
                &local,
                argument.span,
            )?;
        }
        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => Self::quoted_value(argument)?,
                None => self.eval_in(&specification.init_form, &local)?,
            };
            Self::bind_macro_pattern(&specification.pattern, value, &local, form.span)?;
            if let Some(supplied_p) = &specification.supplied_p {
                local.define(supplied_p, Value::boolean(supplied.is_some()));
            }
        }
        if let Some(rest_name) = &lambda_list.rest {
            let rest_values = arguments[key_start..]
                .iter()
                .map(Self::quoted_value)
                .collect::<Result<Vec<_>, _>>()?;
            local.define(rest_name, Value::list(rest_values));
        }

        if let Some(supplied_keywords) = keyword_arguments {
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords.get(&specification.keyword_name);
                let value = match supplied {
                    Some(argument) => Self::quoted_value(argument)?,
                    None => self.eval_in(&specification.init_form, &local)?,
                };
                Self::bind_macro_pattern(&specification.pattern, value, &local, form.span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    local.define(supplied_p, Value::boolean(supplied.is_some()));
                }
            }
        }
        self.bind_macro_auxiliary_parameters(&lambda_list.auxiliary, &local)?;

        Ok(local)
    }

    fn bind_macro_auxiliary_parameters(
        &self,
        specifications: &[MacroAuxiliaryParameter],
        local: &Environment,
    ) -> Result<(), RuntimeError> {
        for specification in specifications {
            let value = self.eval_in(&specification.init_form, local)?;
            local.define(&specification.name, value);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn a_required_destructuring_pattern_that_does_not_match_the_argument_fails() {
        let runtime = Runtime::new();
        let result = runtime.eval_source(
            "(progn (defmacro binding-mismatch ((a b)) (list 'quote (list a b))) (binding-mismatch 1))",
        );
        assert!(result.is_err());
    }
}
