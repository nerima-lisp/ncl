/// Parse a Common Lisp fixed- or general-radix integer literal.
pub fn parse_radix_integer_literal(name: &str) -> Option<i64> {
    let (base, digits_start) = radix_integer_parts(name)?;
    parse_signed_digits(&name[digits_start..], base)
}

/// Parse a Common Lisp floating-point literal using any standard exponent marker.
pub fn parse_float_literal(name: &str) -> Option<f64> {
    if let Ok(value) = name.parse::<f64>() {
        return Some(value);
    }

    let (marker, _) = name
        .char_indices()
        .find(|(_, character)| matches!(character, 's' | 'S' | 'f' | 'F' | 'd' | 'D' | 'l' | 'L'))?;
    let mut normalized = String::with_capacity(name.len());
    normalized.push_str(&name[..marker]);
    normalized.push('e');
    normalized.push_str(&name[marker + 1..]);
    normalized.parse::<f64>().ok()
}

pub(crate) fn is_valid_radix_integer_literal(name: &str) -> bool {
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
        && digits
            .bytes()
            .all(|digit| char::from(digit).to_digit(base).is_some())
}

#[cfg(test)]
mod tests {
    use super::{parse_float_literal, parse_radix_integer_literal};

    #[test]
    fn parses_common_lisp_float_exponent_markers() {
        for literal in [
            "1.25s0",
            "1.25S0",
            "1.25f0",
            "1.25F0",
            "1.25d0",
            "1.25D0",
            "1.25l0",
            "1.25L0",
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
}
