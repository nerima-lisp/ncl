use std::rc::Rc;

use ncl_compiler::{DestructureSpec, Program};
use ncl_syntax::Span;

use super::lambda_list_parameters::{
    destructure_auxiliary_parameters, destructure_keyword_parameters,
    destructure_required_and_optional,
};
use super::pattern::destructure_value;
use crate::vm::primitives::invalid;
use crate::{Environment, Runtime, RuntimeError, Value};

pub(in crate::vm) fn destructure_specification(
    specification: &DestructureSpec,
    value: Value,
    runtime: &Runtime,
    program: &Rc<Program>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    match specification {
        DestructureSpec::Pattern(pattern) => {
            destructure_value(pattern, value, runtime, environment, span)
        }
        DestructureSpec::LambdaList(lambda_list) => {
            let Some(arguments) = value.list_items() else {
                return Err(invalid(
                    "destructuring-bind value must be a proper list",
                    span,
                ));
            };
            if let Some(whole) = &lambda_list.whole {
                runtime.define_in(whole, value, environment);
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
            destructure_required_and_optional(
                lambda_list,
                &arguments[..key_start],
                optional_supplied_count,
                runtime,
                program,
                environment,
                span,
            )?;
            if let Some(rest_name) = &lambda_list.rest {
                runtime.define_in(
                    rest_name,
                    Value::list(arguments[key_start..].to_vec()),
                    environment,
                );
            }
            if lambda_list.has_keyword_section {
                destructure_keyword_parameters(
                    lambda_list,
                    &arguments[key_start..],
                    runtime,
                    program,
                    environment,
                    span,
                )?;
            }
            destructure_auxiliary_parameters(lambda_list, runtime, program, environment, span)
        }
    }
}
