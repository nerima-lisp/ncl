use super::{
    arity, open_binary_input_file, open_binary_io_file, open_binary_output_file, open_input_file,
    open_io_file, open_output_file, pathname_argument, stream_keyword_name,
};
use crate::{RuntimeError, Value};

fn binary_element_type(value: Option<&Value>) -> Result<bool, RuntimeError> {
    let Some(value) = value else {
        return Ok(false);
    };
    if value.symbol_name().is_some_and(|name| {
        name.eq_ignore_ascii_case("CHARACTER") || name.eq_ignore_ascii_case("BASE-CHAR")
    }) {
        return Ok(false);
    }
    let Some(items) = value.list_items() else {
        return Err(RuntimeError::InvalidForm {
            message: "open :element-type must be CHARACTER or (UNSIGNED-BYTE 8)".to_string(),
            span: None,
        });
    };
    let is_unsigned_byte_8 = matches!(
        items.as_slice(),
        [head, Value::Integer(8)]
            if head
                .symbol_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("UNSIGNED-BYTE"))
    ) || matches!(
        items.as_slice(),
        [head, Value::BigInteger(value)]
            if value.as_ref() == &ibig::IBig::from(8)
                && head
                    .symbol_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case("UNSIGNED-BYTE"))
    );
    if is_unsigned_byte_8 {
        Ok(true)
    } else {
        Err(RuntimeError::InvalidForm {
            message: "open :element-type must be CHARACTER or (UNSIGNED-BYTE 8)".to_string(),
            span: None,
        })
    }
}

pub(crate) fn open_file(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("open", "at least 1", arguments.len()));
    }
    if !(arguments.len() - 1).is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: "open requires keyword/value pairs after the pathname".to_string(),
            span: None,
        });
    }
    let path = pathname_argument("open", &arguments[0])?;
    let mut direction = "INPUT".to_string();
    let mut if_does_not_exist = None;
    let mut if_exists = None;
    let mut element_type = None;
    for pair in arguments[1..].as_chunks::<2>().0 {
        let keyword = stream_keyword_name("open", &pair[0])?;
        match keyword.as_str() {
            "DIRECTION" => {
                direction = stream_keyword_name("open :direction", &pair[1])?;
            }
            "IF-DOES-NOT-EXIST" => {
                if_does_not_exist = Some(stream_keyword_name("open :if-does-not-exist", &pair[1])?);
            }
            "IF-EXISTS" => {
                if_exists = Some(stream_keyword_name("open :if-exists", &pair[1])?);
            }
            "ELEMENT-TYPE" => element_type = Some(pair[1].clone()),
            "EXTERNAL-FORMAT" => {}
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("open does not recognize keyword :{keyword}"),
                    span: None,
                });
            }
        }
    }

    let if_does_not_exist = if_does_not_exist.unwrap_or_else(|| {
        if direction == "INPUT" || direction == "IO" {
            "ERROR".to_string()
        } else {
            "CREATE".to_string()
        }
    });
    let if_exists = if_exists.unwrap_or_else(|| "NEW-VERSION".to_string());
    let binary = binary_element_type(element_type.as_ref())?;
    match direction.as_str() {
        "INPUT" if binary => open_binary_input_file(&path, &if_does_not_exist),
        "INPUT" => open_input_file(&path, &if_does_not_exist),
        "OUTPUT" if binary => open_binary_output_file(&path, &if_does_not_exist, &if_exists),
        "OUTPUT" => open_output_file(&path, &if_does_not_exist, &if_exists),
        "PROBE" => {
            if path.exists() {
                Ok(Value::file_probe_stream(path))
            } else {
                Ok(Value::Nil)
            }
        }
        "IO" if binary => open_binary_io_file(&path, &if_does_not_exist, &if_exists),
        "IO" => open_io_file(&path, &if_does_not_exist, &if_exists),
        _ => Err(RuntimeError::InvalidForm {
            message: format!("open received unknown direction :{direction}"),
            span: None,
        }),
    }
}
