use crate::{
    Constant, SymbolTokenKind, normalize_name, parse_float_literal, parse_radix_integer_literal,
    parse_symbol_token,
};

pub fn literal_constant(atom: &str) -> Option<Constant> {
    let token = parse_symbol_token(atom).ok()?;
    match token.kind {
        SymbolTokenKind::Keyword => {
            if token.escaped {
                Some(Constant::KeywordExact(token.name))
            } else {
                Some(Constant::Keyword(normalize_name(&token.name)))
            }
        }
        SymbolTokenKind::Symbol if token.package.is_none() && !token.escaped => {
            if token.name.eq_ignore_ascii_case("nil") || token.name.eq_ignore_ascii_case("#f") {
                return Some(Constant::Nil);
            }
            if token.name.eq_ignore_ascii_case("t") || token.name.eq_ignore_ascii_case("#t") {
                return Some(Constant::Boolean(true));
            }
            if let Some(value) = parse_radix_integer_literal(&token.name) {
                return Some(Constant::Integer(value));
            }
            if let Ok(value) = token.name.parse::<i64>() {
                return Some(Constant::Integer(value));
            }
            if let Some(digits) = big_integer_literal(&token.name) {
                return Some(Constant::BigInteger(digits));
            }
            if let Some((numerator, denominator)) = rational_literal_parts(&token.name) {
                return if denominator == 1 {
                    Some(Constant::Integer(numerator))
                } else {
                    Some(Constant::Rational {
                        numerator,
                        denominator,
                    })
                };
            }
            parse_float_literal(&token.name).map(Constant::Float)
        }
        _ => None,
    }
}

/// Recognizes a decimal integer literal that overflowed `i64` (already
/// tried by the caller), returning its normalized sign-and-digits text.
/// Anything not composed purely of an optional sign followed by digits
/// (e.g. a rational's `/` or a float's `.`/exponent marker) is rejected so
/// those literals keep falling through to their own parsers.
fn big_integer_literal(name: &str) -> Option<String> {
    let digits = name.strip_prefix(['+', '-']).unwrap_or(name);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(
        name.strip_prefix('-')
            .map_or_else(|| digits.to_string(), |digits| format!("-{digits}")),
    )
}

pub(super) fn rational_literal_parts(name: &str) -> Option<(i64, i64)> {
    let (numerator, denominator) = name.split_once('/')?;
    if numerator.is_empty()
        || denominator.is_empty()
        || numerator.contains('/')
        || denominator.contains('/')
    {
        return None;
    }
    let numerator = numerator.parse::<i128>().ok()?;
    let denominator = denominator.parse::<i128>().ok()?;
    if denominator == 0 {
        return None;
    }
    let (numerator, denominator) = if denominator < 0 {
        (numerator.checked_neg()?, denominator.checked_neg()?)
    } else {
        (numerator, denominator)
    };
    let numerator_abs = numerator.unsigned_abs();
    let denominator_abs = u128::try_from(denominator).ok()?;
    let divisor = gcd(numerator_abs, denominator_abs);
    let reduced_numerator = i128::try_from(numerator_abs / divisor).ok()?;
    let reduced_numerator = if numerator < 0 {
        reduced_numerator.checked_neg()?
    } else {
        reduced_numerator
    };
    let reduced_denominator = i128::try_from(denominator_abs / divisor).ok()?;
    Some((
        i64::try_from(reduced_numerator).ok()?,
        i64::try_from(reduced_denominator).ok()?,
    ))
}

const fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
