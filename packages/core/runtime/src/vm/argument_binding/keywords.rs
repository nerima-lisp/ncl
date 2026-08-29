use std::collections::HashMap;
use std::rc::Rc;

use ncl_compiler::{FunctionCode, Program};
use ncl_syntax::Span;

use crate::{Environment, Runtime, RuntimeError, Value};

use super::support::{default_value, define_binding};

pub fn bind_keywords(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    arguments: &[Value],
    key_start: usize,
    local: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if !function.has_keyword_section {
        return Ok(());
    }
    let keyword_arguments = &arguments[key_start..];
    if !keyword_arguments.len().is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: "keyword arguments must be supplied in pairs".to_string(),
            span: Some(span),
        });
    }
    let mut supplied = HashMap::new();
    let mut accepts_unknown = function.allow_other_keys;
    for pair in keyword_arguments.as_chunks::<2>().0 {
        let (Value::Keyword(keyword) | Value::KeywordExact(keyword)) = &pair[0] else {
            return Err(RuntimeError::InvalidForm {
                message: "keyword argument name must be a keyword".to_string(),
                span: Some(span),
            });
        };
        let name = keyword.to_string();
        if name == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
            accepts_unknown = true;
        }
        supplied.insert(name, pair[1].clone());
    }
    if !accepts_unknown
        && let Some(name) = supplied.keys().find(|name| {
            *name != "ALLOW-OTHER-KEYS"
                && !function
                    .keywords
                    .iter()
                    .any(|specification| specification.keyword_name == **name)
        })
    {
        return Err(RuntimeError::InvalidForm {
            message: format!("unknown keyword :{name}"),
            span: Some(span),
        });
    }
    for specification in &function.keywords {
        let value = match supplied.get(&specification.keyword_name) {
            Some(argument) => argument.clone(),
            None => default_value(
                runtime,
                program,
                specification.default_function,
                local,
                span,
                "compiled keyword default is out of range",
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
                Value::boolean(supplied.contains_key(&specification.keyword_name)),
                specification.supplied_p_escaped.unwrap_or(false),
                local,
            );
        }
    }
    Ok(())
}

pub fn bind_auxiliary(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    local: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    for specification in &function.auxiliary {
        let value = default_value(
            runtime,
            program,
            specification.default_function,
            local,
            span,
            "compiled auxiliary default is out of range",
        )?;
        define_binding(
            runtime,
            &specification.name,
            value,
            specification.name_escaped,
            local,
        );
    }
    Ok(())
}
