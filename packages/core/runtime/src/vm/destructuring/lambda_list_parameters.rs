use std::collections::HashMap;
use std::rc::Rc;

use ncl_compiler::{DestructureLambdaList, Program};
use ncl_syntax::Span;

use super::pattern::destructure_value;
use crate::vm::entry::run_code;
use crate::vm::primitives::invalid;
use crate::{Environment, Runtime, RuntimeError, Value};

pub(super) fn destructure_required_and_optional(
    lambda_list: &DestructureLambdaList,
    arguments: &[Value],
    optional_supplied_count: usize,
    runtime: &Runtime,
    program: &Rc<Program>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    for (pattern, argument) in lambda_list.required.iter().zip(arguments.iter().cloned()) {
        destructure_value(pattern, argument, runtime, environment, span)?;
    }
    for (index, parameter) in lambda_list.optional.iter().enumerate() {
        let supplied = (index < optional_supplied_count)
            .then(|| arguments[lambda_list.required.len() + index].clone());
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
        destructure_value(&parameter.pattern, value, runtime, environment, span)?;
        if let Some(supplied_p) = &parameter.supplied_p {
            runtime.define_in(supplied_p, Value::boolean(supplied.is_some()), environment);
        }
    }
    Ok(())
}

pub(super) fn destructure_keyword_parameters(
    lambda_list: &DestructureLambdaList,
    keyword_arguments: &[Value],
    runtime: &Runtime,
    program: &Rc<Program>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if !keyword_arguments.len().is_multiple_of(2) {
        return Err(invalid("keyword arguments must be supplied in pairs", span));
    }
    let mut supplied_keywords = HashMap::new();
    let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
    for pair in keyword_arguments.as_chunks::<2>().0 {
        let (Value::Keyword(keyword) | Value::KeywordExact(keyword)) = &pair[0] else {
            return Err(invalid("keyword argument name must be a keyword", span));
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
        destructure_value(&parameter.pattern, value, runtime, environment, span)?;
        if let Some(supplied_p) = &parameter.supplied_p {
            runtime.define_in(supplied_p, Value::boolean(supplied.is_some()), environment);
        }
    }
    Ok(())
}

pub(super) fn destructure_auxiliary_parameters(
    lambda_list: &DestructureLambdaList,
    runtime: &Runtime,
    program: &Rc<Program>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
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
