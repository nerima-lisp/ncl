/// Parse a Common Lisp fixed- or general-radix integer literal.
#[must_use]
pub fn parse_radix_integer_literal(name: &str) -> Option<i64> {
    let (base, digits_start) = radix_integer_parts(name)?;
    parse_signed_digits(&name[digits_start..], base)
}

/// Parse a valid Common Lisp radix integer and return its normalized decimal text.
/// The caller can parse the returned text with an arbitrary-precision integer type.
#[must_use]
pub fn parse_radix_integer_literal_text(name: &str) -> Option<String> {
    let (base, digits_start) = radix_integer_parts(name)?;
    let digits = &name[digits_start..];
    valid_signed_digits(digits, base).then(|| radix_digits_to_decimal(digits, base))
}

/// Parse a Common Lisp floating-point literal using any standard exponent marker.
#[must_use]
pub fn parse_float_literal(name: &str) -> Option<f64> {
    if let Ok(value) = name.parse::<f64>() {
        return Some(value);
    }

    let (marker, _) = name.char_indices().find(|(_, character)| {
        matches!(character, 's' | 'S' | 'f' | 'F' | 'd' | 'D' | 'l' | 'L')
    })?;
    let mut normalized = String::with_capacity(name.len());
    normalized.push_str(&name[..marker]);
    normalized.push('e');
    normalized.push_str(&name[marker + 1..]);
    normalized.parse::<f64>().ok()
}

pub fn is_valid_radix_integer_literal(name: &str) -> bool {
    let Some((base, digits_start)) = radix_integer_parts(name) else {
        return false;
    };
    valid_signed_digits(&name[digits_start..], base)
}

fn radix_integer_parts(name: &str) -> Option<(u32, usize)> {
    let bytes = name.as_bytes();
    if bytes.first() != Some(&b'#') {
        return None;
    }
    let (base, digits_start) = match bytes.get(1).map(u8::to_ascii_uppercase) {
        Some(b'B') => (2, 2),
        Some(b'O') => (8, 2),
        Some(b'X') => (16, 2),
        _ => general_radix_parts(name)?,
    };

    Some((base, digits_start))
}

fn general_radix_parts(name: &str) -> Option<(u32, usize)> {
    let bytes = name.as_bytes();
    let mut marker_end = 1;
    while bytes.get(marker_end).is_some_and(u8::is_ascii_digit) {
        marker_end += 1;
    }
    if marker_end == 1 || !matches!(bytes.get(marker_end), Some(b'r' | b'R')) {
        return None;
    }

    let base = name[1..marker_end].parse::<u32>().ok()?;
    (2..=36).contains(&base).then_some((base, marker_end + 1))
}

fn parse_signed_digits(digits: &str, base: u32) -> Option<i64> {
    let (negative, digits) = match digits.as_bytes().first() {
        Some(b'+') => (false, &digits[1..]),
        Some(b'-') => (true, &digits[1..]),
        _ => (false, digits),
    };
    if digits.is_empty() || !digits.is_ascii() {
        return None;
    }

    let magnitude = u64::from_str_radix(digits, base).ok()?;
    if !negative {
        i64::try_from(magnitude).ok()
    } else if magnitude == (i64::MAX as u64) + 1 {
        Some(i64::MIN)
    } else {
        i64::try_from(magnitude).ok()?.checked_neg()
    }
}

fn valid_signed_digits(digits: &str, base: u32) -> bool {
    let digits = match digits.as_bytes().first() {
        Some(b'+' | b'-') => &digits[1..],
        _ => digits,
    };
    !digits.is_empty()
        && digits.is_ascii()
        && digits.bytes().all(|digit| char::from(digit).is_digit(base))
}

fn radix_digits_to_decimal(digits: &str, base: u32) -> String {
    let (negative, digits) = match digits.as_bytes().first() {
        Some(b'-') => (true, &digits[1..]),
        Some(b'+') => (false, &digits[1..]),
        _ => (false, digits),
    };
    let mut decimal = String::from("0");
    for digit in digits.bytes() {
        let value = char::from(digit)
            .to_digit(base)
            .unwrap_or_else(|| unreachable!());
        decimal = decimal_mul_add(&decimal, base, value);
    }
    if negative && decimal != "0" {
        format!("-{decimal}")
    } else {
        decimal
    }
}

fn decimal_mul_add(decimal: &str, multiplier: u32, addend: u32) -> String {
    let mut carry = addend;
    let mut result = Vec::with_capacity(decimal.len() + 2);
    for digit in decimal.bytes().rev() {
        let value = u32::from(digit - b'0') * multiplier + carry;
        result.push(b'0' + (value % 10) as u8);
        carry = value / 10;
    }
    while carry > 0 {
        result.push(b'0' + (carry % 10) as u8);
        carry /= 10;
    }
    result.reverse();
    String::from_utf8(result).unwrap_or_else(|_| unreachable!())
}

#[cfg(test)]
mod tests {
    use super::{
        parse_float_literal, parse_radix_integer_literal, parse_radix_integer_literal_text,
    };

    #[test]
    fn parses_common_lisp_float_exponent_markers() {
        for literal in [
            "1.25s0", "1.25S0", "1.25f0", "1.25F0", "1.25d0", "1.25D0", "1.25l0", "1.25L0",
        ] {
            assert_eq!(parse_float_literal(literal), Some(1.25), "{literal}");
        }
        assert_eq!(parse_float_literal("1f2"), Some(100.0));
        assert_eq!(parse_float_literal("-1.5d-1"), Some(-0.15));
    }

    #[test]
    fn rejects_invalid_float_exponent_markers() {
        for literal in ["1f", "1f+", "1f0f0", "symbol"] {
            assert_eq!(parse_float_literal(literal), None, "{literal}");
        }
    }

    #[test]
    fn parses_supported_radices_and_signed_values() {
        assert_eq!(parse_radix_integer_literal("#b101"), Some(5));
        assert_eq!(parse_radix_integer_literal("#O17"), Some(15));
        assert_eq!(parse_radix_integer_literal("#x+Ab"), Some(171));
        assert_eq!(
            parse_radix_integer_literal("#x-8000000000000000"),
            Some(i64::MIN)
        );
        assert_eq!(parse_radix_integer_literal("#2r101"), Some(5));
        assert_eq!(parse_radix_integer_literal("#10R42"), Some(42));
        assert_eq!(parse_radix_integer_literal("#36rZ"), Some(35));
        assert_eq!(
            parse_radix_integer_literal("#16r-8000000000000000"),
            Some(i64::MIN)
        );
    }

    #[test]
    fn rejects_missing_or_invalid_digits_and_overflow() {
        for literal in [
            "#b",
            "#x-",
            "#b2",
            "#o8",
            "#xg",
            "#x1f2gh",
            "#x10000000000000000",
            "#r1",
            "#1r1",
            "#37r1",
            "#10r",
            "#10r-",
            "#2r102",
            "#36r@",
            "#10r18446744073709551616",
        ] {
            assert_eq!(parse_radix_integer_literal(literal), None, "{literal}");
        }
    }

    #[test]
    fn preserves_large_radix_integer_values_as_decimal_text() {
        assert_eq!(
            parse_radix_integer_literal_text("#x10000000000000000"),
            Some("18446744073709551616".to_owned())
        );
        assert_eq!(
            parse_radix_integer_literal_text("#16r-8000000000000000"),
            Some("-9223372036854775808".to_owned())
        );
        assert_eq!(
            parse_radix_integer_literal_text("#b-0"),
            Some("0".to_owned())
        );
    }
}
