use super::{arity, array_option_name, exact, type_error, write_destination};
use crate::{RuntimeError, Value};

pub(crate) fn identity(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "identity", 1)?;
    Ok(arguments[0].clone())
}

pub(crate) fn complement(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "complement", 1)?;
    if !matches!(arguments[0], Value::Function(_)) {
        return Err(type_error("complement", "function", &arguments[0]));
    }
    Ok(Value::complement(arguments[0].clone()))
}

pub(crate) fn constantly(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(crate) fn print_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("print", "1 to 2", arguments.len()));
    }
    let text = printed_value(&arguments[0], true);
    write_destination("print", arguments.get(1), "\n")?;
    write_destination("print", arguments.get(1), &text)?;
    write_destination("print", arguments.get(1), "\n")?;
    Ok(arguments[0].clone())
}

pub(crate) fn princ(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("princ", "1 to 2", arguments.len()));
    }
    let text = printed_value(&arguments[0], false);
    write_destination("princ", arguments.get(1), &text)?;
    Ok(arguments[0].clone())
}

pub(crate) fn prin1(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("prin1", "1 to 2", arguments.len()));
    }
    let text = printed_value(&arguments[0], true);
    write_destination("prin1", arguments.get(1), &text)?;
    Ok(arguments[0].clone())
}

pub(crate) fn write_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(crate) fn printed_value(value: &Value, escape: bool) -> String {
    match value {
        Value::String(value) if !escape => value.to_string(),
        Value::String(value) => format!("{value:?}"),
        Value::Cons(cell) => cell.printed_with(|value| printed_value(value, escape)),
        Value::Vector(values) => {
            let Some(_guard) =
                crate::value::PrintGuard::enter(crate::value::PrintKind::Vector, values.identity())
            else {
                return "#<CIRCULAR>".to_string();
            };
            delimited_values(&values.visible_snapshot(), "#(", ")", escape)
        }
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

#[cfg(test)]
mod cons_safety_tests {
    use super::*;
    use ncl_syntax::{Form, FormKind, Span};

    // These are internal termination checks, not PRINT-CIRCLE conformance tests.
    #[test]
    fn cons_cycles_terminate_printing_and_form_conversion() {
        for through_car in [false, true] {
            let value = Value::cons(Value::Integer(7), Value::Nil);
            let Value::Cons(cell) = &value else {
                unreachable!()
            };
            if through_car {
                cell.set_car(value.clone());
            } else {
                cell.set_cdr(value.clone());
            }
            assert!(value.to_string().contains("#<CIRCULAR>"));
            assert!(format!("{value:?}").contains("#<CIRCULAR>"));
            assert!(format!("{cell:?}").contains("#<CIRCULAR>"));
            for escape in [false, true] {
                assert!(printed_value(&value, escape).contains("#<CIRCULAR>"));
            }
            let span = Span::new(0, 0);
            let form = crate::Runtime::form_from_value(&value, span)
                .unwrap_or_else(|error| panic!("cyclic data must retain its graph: {error}"));
            if through_car {
                assert!(matches!(&form.kind, FormKind::List(items)
                    if items.len() == 1 && matches!(items[0].kind, FormKind::CircularReference)));
            } else {
                assert!(matches!(form.kind, FormKind::CircularReference));
            }
            let retained = crate::Runtime::quoted_value(&form)
                .unwrap_or_else(|error| panic!("cyclic data must be quotable: {error}"));
            assert!(retained.eq_value(&value));
            let runtime = crate::Runtime::new();
            let quoted = Form::list(vec![Form::atom("QUOTE", span), form.clone()], span);
            let result = runtime
                .eval_in(&quoted, &runtime.global_environment())
                .unwrap_or_else(|error| panic!("quoted cyclic data must evaluate: {error}"));
            assert!(result.eq_value(&value));
            assert!(matches!(
                runtime.eval_in(&form, &runtime.global_environment()),
                Err(RuntimeError::InvalidForm { message, span: error_span })
                    if message == "circular cons cannot be converted to a form"
                        && error_span == Some(span)
            ));
            if !through_car {
                assert!(value.list_items().is_none());
                assert!(value.nth_tail(10).is_some_and(|tail| tail.eq_value(&value)));
            }
            cell.set_car(Value::Nil);
            cell.set_cdr(Value::Nil);
        }
    }

    #[test]
    fn mixed_cycles_and_noncyclic_sharing_print_safely() {
        let value = Value::cons(Value::Nil, Value::Nil);
        let Value::Cons(cell) = &value else {
            unreachable!()
        };
        let vector = Value::vector(vec![value.clone()]);
        cell.set_car(vector);
        assert!(printed_value(&value, true).contains("#<CIRCULAR>"));
        let form = crate::Runtime::form_from_value(&value, Span::new(0, 0))
            .unwrap_or_else(|error| panic!("a vector literal must retain its graph: {error}"));
        let ncl_syntax::FormKind::List(items) = &form.kind else {
            unreachable!()
        };
        let retained = crate::Runtime::quoted_value(&items[0])
            .unwrap_or_else(|error| panic!("a retained vector must be readable: {error}"));
        assert!(retained.eq_value(&cell.car()));
        let structure = Value::structure_with_types(
            "node",
            vec![("edge".to_owned(), value.clone())],
            Vec::new(),
        );
        cell.set_car(structure);
        assert!(value.to_string().contains("#<CIRCULAR>"));
        assert!(printed_value(&value, true).contains("#<CIRCULAR>"));
        cell.set_car(Value::Integer(1));
        let shared = Value::list(vec![value.clone(), value]);
        assert_eq!(printed_value(&shared, true), "((1) (1))");
        assert!(crate::Runtime::form_from_value(&shared, Span::new(0, 0)).is_ok());
    }
}
