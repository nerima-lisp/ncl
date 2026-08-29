use std::rc::Rc;

use crate::RuntimeError;
use crate::builtins::builtin_printer::parse_print_options;
use crate::builtins::*;

#[test]
fn core_printing_wrappers_cover_success_and_argument_errors() -> Result<(), RuntimeError> {
    let value = Value::string("hello");
    assert_eq!(
        identity(std::slice::from_ref(&value))?.to_string(),
        "\"hello\""
    );
    assert!(identity(&[]).is_err());
    assert!(identity(&[Value::Nil, Value::Nil]).is_err());

    let type_cases = [
        (Value::Integer(1), "INTEGER"),
        (Value::string("text"), "STRING"),
    ];
    for (input, expected) in type_cases {
        assert_eq!(type_of(&[input])?.to_string(), expected);
    }
    assert!(type_of(&[]).is_err());

    assert_eq!(
        princ(std::slice::from_ref(&value))?.to_string(),
        "\"hello\""
    );
    assert_eq!(
        prin1(std::slice::from_ref(&value))?.to_string(),
        "\"hello\""
    );
    assert_eq!(
        print_value(std::slice::from_ref(&value))?.to_string(),
        "\"hello\""
    );
    for primitive in [print_value, princ, prin1] {
        assert!(primitive(&[]).is_err());
        assert!(primitive(&[Value::Nil, Value::Nil, Value::Nil]).is_err());
    }
    Ok(())
}

#[test]
fn write_wrappers_cover_print_options_and_errors() -> Result<(), RuntimeError> {
    let value = Value::string("hello");
    let cases = [
        (vec![value.clone()], "\"\\\"hello\\\"\""),
        (
            vec![value.clone(), Value::keyword("escape"), Value::Nil],
            "\"hello\"",
        ),
    ];
    for (arguments, expected) in cases {
        assert_eq!(write_to_string(&arguments)?.to_string(), expected);
    }
    assert_eq!(
        write_value(std::slice::from_ref(&value))?.to_string(),
        "\"hello\""
    );
    assert!(write_value(&[]).is_err());
    assert!(write_to_string(&[]).is_err());
    assert!(write_to_string(&[value.clone(), Value::Integer(1)]).is_err());
    assert!(write_to_string(&[value.clone(), Value::keyword("stream"), Value::Nil]).is_err());
    assert!(write_to_string(&[value, Value::keyword("unknown"), Value::Nil]).is_err());
    Ok(())
}

#[test]
fn print_helpers_cover_table_driven_values_and_options() -> Result<(), RuntimeError> {
    let values = [
        (Value::string("text"), "\"text\"", "text"),
        (
            Value::list(vec![Value::Integer(1), Value::Integer(2)]),
            "(1 2)",
            "(1 2)",
        ),
        (
            Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2)),
            "(1 . 2)",
            "(1 . 2)",
        ),
        (
            Value::dotted_list(Vec::new(), Value::Integer(2)),
            "(. 2)",
            "(. 2)",
        ),
        (
            Value::Vector(Rc::new(vec![Value::string("text"), Value::Integer(2)])),
            "#(\"text\" 2)",
            "#(text 2)",
        ),
        (
            Value::list(vec![Value::string("nested")]),
            "(\"nested\")",
            "(nested)",
        ),
    ];
    for (value, escaped, unescaped) in values {
        assert_eq!(printed_value(&value, true), escaped);
        assert_eq!(printed_value(&value, false), unescaped);
        let written = write_to_string(std::slice::from_ref(&value))?;
        assert_eq!(printed_value(&written, false), escaped);
    }

    let (escape, stream) = parse_print_options(
        "write",
        &[
            Value::keyword("escape"),
            Value::Nil,
            Value::keyword("stream"),
            Value::Nil,
        ],
        true,
    )?;
    assert!(!escape);
    assert!(matches!(stream, Some(Value::Nil)));
    Ok(())
}
