use crate::RuntimeError;
use crate::builtins::*;

#[test]
fn character_stream_builtins_cover_peek_unread_and_output_boundaries() -> Result<(), RuntimeError> {
    let input = make_string_input_stream(&[Value::string("  ab")])?;
    assert!(matches!(
        peek_char(std::slice::from_ref(&input))?,
        Value::Character(' ')
    ));
    assert!(matches!(
        peek_char(&[Value::boolean(true), input.clone()])?,
        Value::Character('a')
    ));
    assert!(matches!(
        peek_char(&[Value::Character('b'), input.clone()])?,
        Value::Character('b')
    ));
    assert!(matches!(
        read_char(std::slice::from_ref(&input))?,
        Value::Character('b')
    ));
    assert!(unread_char(&[Value::Character('b'), input.clone()]).is_ok());
    assert!(matches!(
        read_char(std::slice::from_ref(&input))?,
        Value::Character('b')
    ));
    assert!(peek_char(&[Value::Integer(1), input]).is_err());

    let output = make_string_output_stream(&[])?;
    assert!(matches!(
        write_char(&[Value::Character('x'), output.clone()])?,
        Value::Character('x')
    ));
    assert!(matches!(
        write_string(&[Value::string("y"), output.clone()])?,
        Value::String(_)
    ));
    assert!(matches!(terpri(std::slice::from_ref(&output))?, Value::Nil));
    assert!(fresh_line(std::slice::from_ref(&output)).is_ok());
    assert!(matches!(
        get_output_stream_string(&[output])?,
        Value::String(text) if text.as_ref() == "xy\n"
    ));
    Ok(())
}

#[test]
fn character_stream_builtins_cover_eof_states_and_stream_types() -> Result<(), RuntimeError> {
    type Reader = fn(&[Value]) -> Result<Value, RuntimeError>;
    let eof_cases: [(&str, Reader); 2] = [("read-char", read_char), ("read-line", read_line)];
    for (name, operation) in eof_cases {
        let stream = make_string_input_stream(&[Value::string("")])?;
        assert!(
            operation(std::slice::from_ref(&stream)).is_err(),
            "{name} should signal EOF"
        );
    }

    let stream = make_string_input_stream(&[Value::string("")])?;
    assert!(matches!(
        read_char(&[stream.clone(), Value::Nil, Value::Integer(7)])?,
        Value::Integer(7)
    ));
    assert!(matches!(
        peek_char(&[stream, Value::Nil, Value::Integer(8)])?,
        Value::Integer(8)
    ));

    let output = make_string_output_stream(&[])?;
    assert!(read_char(std::slice::from_ref(&output)).is_err());
    assert!(write_char(&[Value::Character('x'), Value::Nil]).is_ok());
    assert!(write_string(&[Value::string("x"), Value::Integer(1)]).is_err());
    assert!(fresh_line(&[Value::Integer(1)]).is_err());
    assert!(write_line(&[Value::string("x"), Value::Integer(1)]).is_err());
    Ok(())
}
