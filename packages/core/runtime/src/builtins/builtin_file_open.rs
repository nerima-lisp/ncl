use super::{
    arity, open_input_file, open_io_file, open_output_file, pathname_argument, stream_keyword_name,
};
use crate::{RuntimeError, Value};

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
    let mut byte = false;
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
            "ELEMENT-TYPE" => {
                if let Some(items) = pair[1].list_items() {
                    if items.len() == 2 && items[0].symbol_name() == Some("UNSIGNED-BYTE") && matches!(&items[1], Value::Integer(8)) { byte = true; continue; }
                }
                let element_type = pair[1].symbol_name().ok_or_else(|| RuntimeError::InvalidForm {
                    message: "open :element-type currently supports only CHARACTER".to_string(),
                    span: None,
                })?;
                if element_type != "CHARACTER" {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("open does not support :element-type {element_type}"),
                        span: None,
                    });
                }
            }
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
    match direction.as_str() {
        "INPUT" => open_input_file(&path, &if_does_not_exist, byte),
        "OUTPUT" => open_output_file(&path, &if_does_not_exist, &if_exists, byte),
        "PROBE" => {
            if path.exists() {
                Ok(Value::file_input_stream(""))
            } else {
                Ok(Value::Nil)
            }
        }
        "IO" => open_io_file(&path, &if_does_not_exist, &if_exists, byte),
        _ => Err(RuntimeError::InvalidForm {
            message: format!("open received unknown direction :{direction}"),
            span: None,
        }),
    }
}
