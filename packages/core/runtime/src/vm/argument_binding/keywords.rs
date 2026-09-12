use std::collections::HashMap;
use std::rc::Rc;

use ncl_compiler::{FunctionCode, Program};

use crate::{Runtime, RuntimeError, Value};

use super::support::{default_value, define_binding};
use super::{BindingContext, declare_special_if};

pub fn bind_keywords(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    arguments: &[Value],
    key_start: usize,
    context: &mut BindingContext<'_>,
) -> Result<(), RuntimeError> {
    if !function.has_keyword_section {
        return Ok(());
    }
    let keyword_arguments = &arguments[key_start..];
    if !keyword_arguments.len().is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: "keyword arguments must be supplied in pairs".to_string(),
            span: Some(context.span),
        });
    }
    let mut supplied = HashMap::new();
    for pair in keyword_arguments.as_chunks::<2>().0 {
        let name = match &pair[0] {
            Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword.to_string(),
            Value::InternedSymbol(symbol) if symbol.keyword() => symbol.name().to_string(),
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: "keyword argument name must be a keyword".to_string(),
                    span: Some(context.span),
                });
            }
        };
        supplied.entry(name).or_insert_with(|| pair[1].clone());
    }
    let accepts_unknown = function.allow_other_keys
        || supplied
            .get("ALLOW-OTHER-KEYS")
            .is_some_and(Value::is_truthy);
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
            span: Some(context.span),
        });
    }
    for specification in &function.keywords {
        let value = match supplied.get(&specification.keyword_name) {
            Some(argument) => argument.clone(),
            None => default_value(
                runtime,
                program,
                specification.default_function,
                &context.local,
                context.span,
                "compiled keyword default is out of range",
            )?,
        };
        context.local = context.local.child();
        declare_special_if(
            &context.local,
            &specification.name,
            specification.name_escaped,
            context.special_names,
        );
        define_binding(
            runtime,
            &specification.name,
            value,
            specification.name_escaped,
            &context.local,
        );
        if let Some(name) = &specification.supplied_p {
            declare_special_if(
                &context.local,
                name,
                specification.supplied_p_escaped.unwrap_or(false),
                context.special_names,
            );
            define_binding(
                runtime,
                name,
                Value::boolean(supplied.contains_key(&specification.keyword_name)),
                specification.supplied_p_escaped.unwrap_or(false),
                &context.local,
            );
        }
    }
    Ok(())
}

pub fn bind_auxiliary(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    context: &mut BindingContext<'_>,
) -> Result<(), RuntimeError> {
    for specification in &function.auxiliary {
        let value = default_value(
            runtime,
            program,
            specification.default_function,
            &context.local,
            context.span,
            "compiled auxiliary default is out of range",
        )?;
        context.local = context.local.child();
        declare_special_if(
            &context.local,
            &specification.name,
            specification.name_escaped,
            context.special_names,
        );
        define_binding(
            runtime,
            &specification.name,
            value,
            specification.name_escaped,
            &context.local,
        );
    }
    Ok(())
}
