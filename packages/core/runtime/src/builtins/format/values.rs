
fn format_argument<'a>(
    directive: &str,
    arguments: &'a [Value],
    argument_index: &mut usize,
) -> Result<&'a Value, RuntimeError> {
    let argument = arguments
        .get(*argument_index)
        .ok_or_else(|| RuntimeError::InvalidForm {
            message: format!("format directive {directive} needs another argument"),
            span: None,
        })?;
    *argument_index += 1;
    Ok(argument)
}

fn append_aesthetic(output: &mut String, value: &Value) {
    match value {
        Value::String(value) => output.push_str(value),
        Value::Character(value) => output.push(*value),
        Value::List(values) => {
            output.push('(');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                append_aesthetic(output, value);
            }
            output.push(')');
        }
        Value::DottedList { items, tail } => {
            output.push('(');
            for (index, value) in items.iter().enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                append_aesthetic(output, value);
            }
            if !items.is_empty() {
                output.push(' ');
            }
            output.push_str(". ");
            append_aesthetic(output, tail);
            output.push(')');
        }
        Value::Vector { .. } => {
            let values = value.vector_items().expect("vector items");
            output.push_str("#(");
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                append_aesthetic(output, value);
            }
            output.push(')');
        }
        _ => output.push_str(&value.to_string()),
    }
}

fn format_integer_radix(value: i64, radix: u32) -> String {
    let mut result = format_unsigned_integer(value.unsigned_abs(), radix);
    if value < 0 {
        result.insert(0, '-');
    }
    result
}

fn format_unsigned_integer(mut magnitude: u64, radix: u32) -> String {
    const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    if magnitude == 0 {
        return "0".to_string();
    }
    let mut digits = Vec::new();
    while magnitude != 0 {
        digits.push(DIGITS[(magnitude % u64::from(radix)) as usize] as char);
        magnitude /= u64::from(radix);
    }
    digits.iter().rev().collect()
}
