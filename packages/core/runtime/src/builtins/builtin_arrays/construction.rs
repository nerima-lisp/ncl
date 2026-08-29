use crate::builtins::{
    arity, array_option_name, array_total_size_for, flatten_array_contents, parse_array_dimensions,
};
use crate::{RuntimeError, Value};

#[expect(clippy::unnecessary_wraps)]
pub fn vector(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::vector(arguments.to_vec()))
}

pub fn make_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-array", "at least one", 0));
    }
    let dimensions = parse_array_dimensions("make-array", &arguments[0])?;
    let mut initial_element = None;
    let mut initial_contents = None;
    if !(arguments.len() - 1).is_multiple_of(2) {
        return Err(arity(
            "make-array",
            "one dimension and keyword/value pairs",
            arguments.len(),
        ));
    }
    for pair in arguments[1..].as_chunks::<2>().0 {
        let name = array_option_name("make-array", &pair[0])?;
        match name.as_str() {
            "INITIAL-ELEMENT" => {
                if initial_contents.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-array cannot combine :initial-element and :initial-contents"
                            .to_string(),
                        span: None,
                    });
                }
                initial_element = Some(pair[1].clone());
            }
            "INITIAL-CONTENTS" => {
                if initial_element.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-array cannot combine :initial-element and :initial-contents"
                            .to_string(),
                        span: None,
                    });
                }
                initial_contents = Some(pair[1].clone());
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-array does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    let total_size = array_total_size_for("make-array", &dimensions)?;
    let elements = if let Some(contents) = initial_contents {
        let mut elements = Vec::with_capacity(total_size);
        flatten_array_contents("make-array", &contents, &dimensions, &mut elements)?;
        elements
    } else {
        vec![initial_element.unwrap_or(Value::Nil); total_size]
    };
    if dimensions.len() == 1 {
        Ok(Value::vector(elements))
    } else {
        Ok(Value::array(dimensions, elements))
    }
}
