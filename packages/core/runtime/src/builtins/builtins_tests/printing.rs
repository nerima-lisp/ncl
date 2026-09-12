use crate::RuntimeError;
use crate::builtins::builtin_printer::parse_print_options;
use crate::builtins::*;

#[test]
fn shared_cycles_printing_internal_safety_not_print_circle() {
    let vector = Value::vector(vec![Value::Nil, Value::Nil]);
    let Value::Vector(storage) = &vector else {
        unreachable!()
    };
    assert!(storage.set(0, vector.clone()));
    assert!(storage.set(1, vector.clone()));
    let expected = "#(#<CIRCULAR> #<CIRCULAR>)";
    assert_eq!(vector.to_string(), expected);
    assert_eq!(format!("{vector:?}"), format!("Value({expected})"));
    for escape in [false, true] {
        assert_eq!(printed_value(&vector, escape), expected);
    }
    assert!(storage.set(0, Value::Nil));
    assert!(storage.set(1, Value::Nil));
    let child = Value::vector(vec![Value::Integer(1)]);
    let shared = Value::vector(vec![child.clone(), child]);
    assert_eq!(shared.to_string(), "#(#(1) #(1))");
    assert_eq!(printed_value(&shared, true), "#(#(1) #(1))");
}

#[test]
fn vector_structure_cycle_printing_internal_safety() {
    let vector = Value::vector(vec![Value::Nil]);
    let structure = Value::structure_with_types(
        "NODE",
        vec![("LINK".to_owned(), vector.clone())],
        Vec::new(),
    );
    let Value::Vector(storage) = &vector else {
        unreachable!()
    };
    assert!(storage.set(0, structure.clone()));
    for value in [&vector, &structure] {
        assert!(value.to_string().contains("#<CIRCULAR>"));
        assert!(format!("{value:?}").contains("#<CIRCULAR>"));
        assert!(printed_value(value, true).contains("#<CIRCULAR>"));
    }
    assert!(storage.set(0, Value::Nil));
}

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
        (Value::dotted_list(Vec::new(), Value::Integer(2)), "2", "2"),
        (
            Value::vector(vec![Value::string("text"), Value::Integer(2)]),
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
