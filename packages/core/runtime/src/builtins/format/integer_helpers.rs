#[allow(clippy::wildcard_imports)]
use super::*;

pub(crate) fn append_aesthetic(output: &mut String, value: &Value) {
    match value {
        Value::String(value) => output.push_str(value),
        Value::Character(value) => output.push(*value),
        Value::Cons(cell) => output.push_str(&cell.printed_with(|value| {
            let mut text = String::new();
            append_aesthetic(&mut text, value);
            text
        })),
        Value::Vector(values) => {
            let Some(_guard) =
                crate::value::PrintGuard::enter(crate::value::PrintKind::Vector, values.identity())
            else {
                output.push_str("#<CIRCULAR>");
                return;
            };
            output.push_str("#(");
            for (index, value) in values.snapshot().iter().enumerate() {
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

pub(crate) fn format_integer_radix(value: &ibig::IBig, radix: u32) -> String {
    let negative = value < &ibig::IBig::from(0);
    let magnitude = if negative {
        -value.clone()
    } else {
        value.clone()
    };
    let mut result = format_unsigned_integer(&magnitude, radix);
    if negative {
        result.insert(0, '-');
    }
    result
}

pub(crate) fn format_unsigned_integer(magnitude: &ibig::IBig, radix: u32) -> String {
    if magnitude == &ibig::IBig::from(0) {
        return "0".to_string();
    }
    let radix_value = ibig::IBig::from(radix);
    let mut remainder = magnitude.clone();
    let mut digits = Vec::new();
    while remainder != ibig::IBig::from(0) {
        let digit_value = &remainder % &radix_value;
        let digit = usize::try_from(&digit_value).unwrap_or_default();
        digits.push(FORMAT_DIGITS[digit] as char);
        remainder = &remainder / &radix_value;
    }
    digits.iter().rev().collect()
}
