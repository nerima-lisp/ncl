use super::{arity, array_option_name, exact, type_error, write_destination};
use crate::{RuntimeError, Value};

pub(crate) fn identity(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "identity", 1)?;
    Ok(arguments[0].clone())
}

pub(super) fn complement(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "complement", 1)?;
    if !matches!(
        arguments[0],
        Value::Function(_)
            | Value::Symbol(_)
            | Value::SymbolExact(_)
            | Value::UninternedSymbol(_)
            | Value::Keyword(_)
            | Value::KeywordExact(_)
    ) {
        return Err(type_error("complement", "function", &arguments[0]));
    }
    Ok(Value::complement(arguments[0].clone()))
}

pub(super) fn constantly(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "constantly", 1)?;
    Ok(Value::constantly(arguments[0].clone()))
}

pub(crate) fn type_of(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "type-of", 1)?;
    Ok(Value::symbol(
        arguments[0]
            .structure_name()
            .unwrap_or_else(|| arguments[0].type_name()),
    ))
}

pub(super) fn print_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("print", "1 to 2", arguments.len()));
    }
    let text = printed_value(&arguments[0], true);
    write_destination("print", arguments.get(1), "\n")?;
    write_destination("print", arguments.get(1), &text)?;
    write_destination("print", arguments.get(1), "\n")?;
    Ok(arguments[0].clone())
}

pub(super) fn princ(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("princ", "1 to 2", arguments.len()));
    }
    let text = printed_value(&arguments[0], false);
    write_destination("princ", arguments.get(1), &text)?;
    Ok(arguments[0].clone())
}

pub(super) fn prin1(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("prin1", "1 to 2", arguments.len()));
    }
    let text = printed_value(&arguments[0], true);
    write_destination("prin1", arguments.get(1), &text)?;
    Ok(arguments[0].clone())
}

pub(super) fn write_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("write", "at least 1", arguments.len()));
    }
    let (escape, stream) = parse_print_options("write", &arguments[1..], true)?;
    let text = printed_value(&arguments[0], escape);
    write_destination("write", stream.as_ref(), &text)?;
    Ok(arguments[0].clone())
}

pub(crate) fn write_to_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("write-to-string", "at least 1", arguments.len()));
    }
    let (escape, _) = parse_print_options("write-to-string", &arguments[1..], false)?;
    Ok(Value::string(printed_value(&arguments[0], escape)))
}

pub fn parse_print_options(
    function: &str,
    options: &[Value],
    allow_stream: bool,
) -> Result<(bool, Option<Value>), RuntimeError> {
    if !options.len().is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} requires keyword/value pairs"),
            span: None,
        });
    }
    let mut escape = true;
    let mut stream = None;
    for pair in options.as_chunks::<2>().0 {
        let name = array_option_name(function, &pair[0])?;
        match name.as_str() {
            "ESCAPE" => escape = pair[1].is_truthy(),
            "STREAM" if allow_stream => stream = Some(pair[1].clone()),
            "STREAM" => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not support keyword :stream"),
                    span: None,
                });
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    Ok((escape, stream))
}

pub(super) fn printed_value(value: &Value, escape: bool) -> String {
    match value {
        Value::String(value) if !escape => value.to_string(),
        Value::String(value) => format!("{value:?}"),
        Value::List(values) => delimited_values(values, "(", ")", escape),
        Value::DottedList { items, tail } => {
            let mut text = String::from("(");
            if !items.is_empty() {
                text.push_str(
                    &items
                        .iter()
                        .map(|value| printed_value(value, escape))
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                text.push(' ');
            }
            text.push_str(". ");
            text.push_str(&printed_value(tail, escape));
            text.push(')');
            text
        }
        Value::Vector(values) => delimited_values(&values.borrow(), "#(", ")", escape),
        _ => value.to_string(),
    }
}

fn delimited_values(values: &[Value], opening: &str, closing: &str, escape: bool) -> String {
    let contents = values
        .iter()
        .map(|value| printed_value(value, escape))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{opening}{contents}{closing}")
}
