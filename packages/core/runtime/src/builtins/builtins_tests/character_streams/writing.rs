use crate::RuntimeError;
use crate::builtins::*;

#[test]
fn writing_builtins_reject_bad_arity_and_argument_types() -> Result<(), RuntimeError> {
    assert!(write_char(&[Value::Character('a'), Value::Nil, Value::Nil]).is_err());
    assert!(write_string(&[Value::string("a"), Value::Nil, Value::Nil]).is_err());
    assert!(terpri(&[Value::Nil, Value::Nil]).is_err());
    assert!(fresh_line(&[Value::Nil, Value::Nil]).is_err());
    assert!(write_line(&[Value::string("a"), Value::Nil, Value::Nil]).is_err());

    assert!(write_char(&[Value::Integer(1)]).is_err());
    assert!(write_string(&[Value::Integer(1)]).is_err());
    assert!(write_line(&[Value::Integer(1)]).is_err());

    let input = make_string_input_stream(&[Value::string("z")])?;
    assert!(
        write_char(&[Value::Character('z'), input.clone()]).is_err(),
        "write-char rejects an input-only stream destination"
    );
    assert!(
        write_string(&[Value::string("z"), input]).is_err(),
        "write-string rejects an input-only stream destination"
    );

    assert!(matches!(fresh_line(&[])?, Value::Boolean(true)));
    Ok(())
}
