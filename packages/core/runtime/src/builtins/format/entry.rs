#[allow(clippy::wildcard_imports)]
use super::*;

pub fn format_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("format", "at least 2", arguments.len()));
    }
    let control = match &arguments[1] {
        Value::String(value) => value.as_ref(),
        value => return Err(type_error("format", "a string control", value)),
    };
    let output = format_control(control, &arguments[2..])?;
    match &arguments[0] {
        Value::Nil => Ok(Value::string(output)),
        Value::Boolean(true) => {
            print!("{output}");
            Ok(Value::Nil)
        }
        Value::Stream(stream) => {
            if stream.borrow_mut().write(&output) {
                Ok(Value::Nil)
            } else {
                Err(stream_state_error("format", "an open output stream"))
            }
        }
        value => Err(type_error("format", "NIL or T as the destination", value)),
    }
}

pub fn format_control(control: &str, arguments: &[Value]) -> Result<String, RuntimeError> {
    let characters = control.chars().collect::<Vec<_>>();
    let (output, _, _) = format_control_characters(&characters, arguments, false)?;
    Ok(output)
}
