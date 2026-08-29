#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn append_aesthetic(output: &mut String, value: &Value) {
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
        Value::Vector(values) => {
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

pub(super) fn format_integer_radix(value: i64, radix: u32) -> String {
    let mut result = format_unsigned_integer(value.unsigned_abs(), radix);
    if value < 0 {
        result.insert(0, '-');
    }
    result
}

pub(super) fn format_unsigned_integer(mut magnitude: u64, radix: u32) -> String {
    if magnitude == 0 {
        return "0".to_string();
    }
    let mut digits = Vec::new();
    while magnitude != 0 {
        let digit = usize::try_from(magnitude % u64::from(radix)).unwrap_or_default();
        digits.push(FORMAT_DIGITS[digit] as char);
        magnitude /= u64::from(radix);
    }
    digits.iter().rev().collect()
}
