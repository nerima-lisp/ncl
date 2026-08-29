use std::rc::Rc;

use ncl_compiler::{FunctionCode, Program};
use ncl_syntax::Span;

use crate::{Environment, Runtime, RuntimeError, Value};

use super::support::{default_value, define_binding};

pub fn argument_layout(
    function: &FunctionCode,
    arguments: &[Value],
) -> Result<(usize, usize), RuntimeError> {
    let required_count = function.parameters.len();
    let optional_count = function.optional.len();
    let maximum_count = required_count + optional_count;
    let function_name = function
        .name
        .as_deref()
        .unwrap_or("compiled function")
        .to_string();
    if arguments.len() < required_count {
        let expected =
            if optional_count > 0 || function.rest.is_some() || function.has_keyword_section {
                format!("at least {required_count}")
            } else {
                required_count.to_string()
            };
        return Err(RuntimeError::Arity {
            function: function_name,
            expected,
            actual: arguments.len(),
        });
    }
    let optional_supplied_count =
        supplied_optional_count(function, arguments, required_count, optional_count);
    let key_start = required_count + optional_supplied_count;
    if !function.has_keyword_section && function.rest.is_none() && arguments.len() > maximum_count {
        let expected = if optional_count > 0 {
            format!("at most {maximum_count}")
        } else {
            maximum_count.to_string()
        };
        return Err(RuntimeError::Arity {
            function: function_name,
            expected,
            actual: arguments.len(),
        });
    }
    Ok((optional_supplied_count, key_start))
}

fn supplied_optional_count(
    function: &FunctionCode,
    arguments: &[Value],
    required_count: usize,
    optional_count: usize,
) -> usize {
    let supplied_count = arguments
        .len()
        .saturating_sub(required_count)
        .min(optional_count);
    if !function.has_keyword_section {
        return supplied_count;
    }
    (0..supplied_count)
        .take_while(|index| {
            !matches!(
                arguments[required_count + *index],
                Value::Keyword(_) | Value::KeywordExact(_)
            )
        })
        .count()
}

pub fn bind_required(
    runtime: &Runtime,
    function: &FunctionCode,
    arguments: &[Value],
    local: &Environment,
) {
    for (index, (parameter, argument)) in function.parameters.iter().zip(arguments).enumerate() {
        if function
            .required_escaped
            .get(index)
            .copied()
            .unwrap_or(false)
        {
            runtime.define_exact_in(parameter, argument.clone(), local);
        } else {
            runtime.define_in(parameter, argument.clone(), local);
        }
    }
}

pub fn bind_optional(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    arguments: &[Value],
    supplied_count: usize,
    local: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    for (index, specification) in function.optional.iter().enumerate() {
        let supplied =
            (index < supplied_count).then(|| &arguments[function.parameters.len() + index]);
        let value = match supplied {
            Some(argument) => argument.clone(),
            None => default_value(
                runtime,
                program,
                specification.default_function,
                local,
                span,
                "compiled optional default is out of range",
            )?,
        };
        define_binding(
            runtime,
            &specification.name,
            value,
            specification.name_escaped,
            local,
        );
        if let Some(name) = &specification.supplied_p {
            define_binding(
                runtime,
                name,
                Value::boolean(supplied.is_some()),
                specification.supplied_p_escaped.unwrap_or(false),
                local,
            );
        }
    }
    Ok(())
}

pub fn bind_rest(
    runtime: &Runtime,
    function: &FunctionCode,
    arguments: &[Value],
    key_start: usize,
    local: &Environment,
) {
    if let Some(name) = &function.rest {
        define_binding(
            runtime,
            name,
            Value::list(arguments[key_start..].to_vec()),
            function.rest_escaped,
            local,
        );
    }
}
