use crate::builtins::*;
use crate::RuntimeError;

#[test]
fn reader_and_stream_builtins_cover_bounds_and_eof_modes() -> Result<(), RuntimeError> {
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
    assert!(make_string_input_stream(&[]).is_err());
    assert!(make_string_input_stream(&[Value::Integer(1)]).is_err());
    assert!(make_string_input_stream(&[Value::string("abc"), Value::Integer(-1)]).is_err());
    assert!(
        make_string_input_stream(&[Value::string("abc"), Value::Integer(2), Value::Integer(1)])
            .is_err()
    );
    assert!(stream_bound("test", &Value::Integer(4), 3).is_err());
    Ok(())
}
