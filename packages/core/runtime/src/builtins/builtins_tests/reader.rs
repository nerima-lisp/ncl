use crate::RuntimeError;
use crate::builtins::*;

#[test]
fn read_from_string_covers_arity_and_start_end_bounds() -> Result<(), RuntimeError> {
    assert!(read_from_string(&[]).is_err());
    assert!(read_from_string(&[Value::Integer(1)]).is_err());
    let parsed = read_from_string(&[Value::string("1 2")])?;
    let parsed_values = parsed.multiple_values();
    assert!(matches!(
        parsed_values.as_slice(),
        [Value::Integer(1), Value::Integer(2)]
    ));
    let parsed = read_from_string(&[Value::string("  1"), Value::Nil, Value::Nil])?;
    let parsed_values = parsed.multiple_values();
    assert_eq!(
        parsed_values.get(1).and_then(|value| match value {
            Value::Integer(position) => Some(*position),
            _ => None,
        }),
        Some(3)
    );
    assert!(read_from_string(&[Value::string(""), Value::Nil, Value::symbol("eof")]).is_ok());
    assert!(read_from_string(&[Value::string(""), Value::boolean(true)]).is_err());
    assert!(
        read_from_string(&[
            Value::string("1"),
            Value::Nil,
            Value::Nil,
            Value::keyword("start")
        ])
        .is_err()
    );
    assert!(
        read_from_string(&[
            Value::string("1"),
            Value::Nil,
            Value::Nil,
            Value::Integer(0),
            Value::Nil
        ])
        .is_err()
    );
    assert!(
        read_from_string(&[
            Value::string("1"),
            Value::Nil,
            Value::Nil,
            Value::keyword("start"),
            Value::Integer(1),
            Value::keyword("end"),
            Value::Integer(1)
        ])
        .is_ok()
    );
    assert!(
        read_from_string(&[
            Value::string("1 2"),
            Value::Nil,
            Value::symbol("eof"),
            Value::keyword("start"),
            Value::Integer(2),
            Value::keyword("end"),
            Value::Integer(3),
            Value::keyword("preserve-whitespace"),
            Value::boolean(true)
        ])
        .is_ok()
    );
    assert!(
        read_from_string(&[
            Value::string("1"),
            Value::Nil,
            Value::Nil,
            Value::keyword("unknown"),
            Value::Nil
        ])
        .is_err()
    );
    assert!(
        read_from_string(&[
            Value::string("1"),
            Value::Nil,
            Value::Nil,
            Value::keyword("start"),
            Value::Integer(2),
            Value::keyword("end"),
            Value::Integer(1)
        ])
        .is_err()
    );
    assert!(
        read_from_string(&[
            Value::string("12345"),
            Value::Nil,
            Value::Nil,
            Value::keyword("start"),
            Value::Integer(3),
            Value::keyword("end"),
            Value::Integer(1)
        ])
        .is_err(),
        "an in-bounds start greater than end is rejected"
    );
    Ok(())
}

#[test]
fn make_string_input_stream_and_stream_bound_reject_invalid_arguments() {
    assert!(make_string_input_stream(&[]).is_err());
    assert!(make_string_input_stream(&[Value::Integer(1)]).is_err());
    assert!(make_string_input_stream(&[Value::string("abc"), Value::Integer(-1)]).is_err());
    assert!(
        make_string_input_stream(&[Value::string("abc"), Value::Integer(2), Value::Integer(1)])
            .is_err()
    );
    assert!(stream_bound("test", &Value::Integer(4), 3).is_err());
}

#[test]
fn read_builtins_reject_bad_arity_and_stream_arguments() -> Result<(), RuntimeError> {
    let five_args = [Value::Nil, Value::Nil, Value::Nil, Value::Nil, Value::Nil];
    assert!(read(&five_args).is_err(), "read takes at most 4 arguments");
    assert!(
        read_preserving_whitespace(&five_args).is_err(),
        "read-preserving-whitespace takes at most 4 arguments"
    );

    assert!(read(&[]).is_err(), "read requires an explicit input stream");
    assert!(read(&[Value::Nil]).is_err(), "read rejects NIL as a stream");
    assert!(
        read(&[Value::Integer(1)]).is_err(),
        "read rejects a non-stream argument"
    );

    let output = make_string_output_stream(&[])?;
    assert!(
        read(std::slice::from_ref(&output)).is_err(),
        "read rejects an output-only stream"
    );

    let empty_input = make_string_input_stream(&[Value::string("")])?;
    assert!(
        read(std::slice::from_ref(&empty_input)).is_err(),
        "read signals EOF by default on an empty stream"
    );

    let closed_input = make_string_input_stream(&[Value::string("1")])?;
    close_stream(std::slice::from_ref(&closed_input))?;
    assert!(
        read(&[closed_input]).is_err(),
        "read rejects an already-closed input stream"
    );
    Ok(())
}
