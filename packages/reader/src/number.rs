//! Number token parsing: potential-number recognition and construction.

use ncl_object::{Runtime, ThreadContext, Word, make_bignum_from_i128, make_double, make_ratio};

use crate::error::ReadError;
use crate::reader::FloatFormat;

/// The largest fixnum value (`most-positive-fixnum`).
const MAX_FIXNUM: i128 = (1_i128 << 62) - 1;
/// The smallest fixnum value (`most-negative-fixnum`).
const MIN_FIXNUM: i128 = -(1_i128 << 62);

/// Numeric value of a digit character in a base, or `None`.
pub fn digit_value(ch: char, base: u32) -> Option<u32> {
    let value = match ch {
        '0'..='9' => u32::from(ch) - u32::from('0'),
        'a'..='z' => u32::from(ch) - u32::from('a') + 10,
        'A'..='Z' => u32::from(ch) - u32::from('A') + 10,
        _ => return None,
    };
    (value < base).then_some(value)
}

/// The shape of a potential-number token and its split points.
#[derive(Clone, Copy, Debug)]
enum NumberShape {
    /// `[sign] digit+ [.]`; `value` is the index of the first digit.
    Integer,
    /// `[sign] digit+ / [sign] digit+`.
    Ratio,
    /// A float with optional exponent; split points are byte-free char indices.
    Float,
}

/// Scan a token and return its shape, or `None` when it is not a potential number.
fn scan_number(token: &[char], base: u32) -> Option<NumberShape> {
    if token.is_empty() {
        return None;
    }
    let mut index = 0;
    let n = token.len();

    if index < n && (token[index] == '+' || token[index] == '-') {
        index += 1;
    }

    let start = index;
    let mut digits_before = 0_usize;
    while index < n && digit_value(token[index], base).is_some() {
        digits_before += 1;
        index += 1;
    }

    if index < n && token[index] == '/' {
        if digits_before == 0 {
            return None;
        }
        index += 1;
        if index < n && (token[index] == '+' || token[index] == '-') {
            index += 1;
        }
        let mut digits_after = 0_usize;
        while index < n && digit_value(token[index], base).is_some() {
            digits_after += 1;
            index += 1;
        }
        if digits_after == 0 || index != n {
            return None;
        }
        return Some(NumberShape::Ratio);
    }

    let mut has_dot = false;
    let mut has_digits_after_dot = false;
    if index < n && token[index] == '.' {
        has_dot = true;
        index += 1;
        while index < n && digit_value(token[index], base).is_some() {
            has_digits_after_dot = true;
            index += 1;
        }
    }

    let mut has_exponent = false;
    if base == 10
        && let Some(marker) = token.get(index).copied()
        && matches!(
            marker,
            'e' | 'E' | 's' | 'S' | 'f' | 'F' | 'd' | 'D' | 'l' | 'L'
        )
    {
        has_exponent = true;
        index += 1;
        if index < n && (token[index] == '+' || token[index] == '-') {
            index += 1;
        }
        let mut exp_digits = 0_usize;
        while index < n && digit_value(token[index], 10).is_some() {
            exp_digits += 1;
            index += 1;
        }
        if exp_digits == 0 || index != n {
            return None;
        }
    }

    if index != n {
        return None;
    }

    if has_dot && has_digits_after_dot {
        return Some(NumberShape::Float);
    }
    if has_exponent {
        return Some(NumberShape::Float);
    }
    if has_dot && !has_digits_after_dot {
        // A trailing dot is an integer: "123." reads as 123.
        return (digits_before > 0).then_some(NumberShape::Integer);
    }
    if digits_before == 0 {
        return None;
    }
    let _ = start;
    Some(NumberShape::Integer)
}

/// Parse a potential-number token into a number object, or `None` when the
/// token is not a number (the caller then reads a symbol).
///
/// # Errors
/// Returns a [`ReadError`] when the token is numeric but malformed or its value
/// cannot be represented.
pub fn parse_number(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    token: &[char],
    base: u32,
    float_format: FloatFormat,
) -> Result<Option<Word>, ReadError> {
    let Some(shape) = scan_number(token, base) else {
        return Ok(None);
    };
    match shape {
        NumberShape::Ratio => {
            let slash = token
                .iter()
                .position(|&c| c == '/')
                .ok_or_else(|| ReadError::InvalidNumber(collect(token)))?;
            let numerator = parse_integer_chars(ctx, runtime, &token[..slash], base)?;
            let denominator = parse_integer_chars(ctx, runtime, &token[slash + 1..], base)?;
            Ok(Some(
                make_ratio(ctx, runtime, numerator, denominator)?.into(),
            ))
        }
        NumberShape::Integer => {
            let integer = parse_integer_chars(ctx, runtime, token, base)?;
            Ok(Some(integer))
        }
        NumberShape::Float => Ok(Some(parse_float(ctx, runtime, token, float_format)?)),
    }
}

/// Parse a signed integer token (with an optional trailing dot) in `base`.
pub fn parse_integer_chars(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    token: &[char],
    base: u32,
) -> Result<Word, ReadError> {
    let mut negative = false;
    let mut index = 0;
    if let Some(&first) = token.first()
        && (first == '+' || first == '-')
    {
        negative = first == '-';
        index += 1;
    }
    let mut value: i128 = 0;
    let mut digits = 0_usize;
    while index < token.len() {
        let ch = token[index];
        if ch == '.' {
            break;
        }
        let digit = digit_value(ch, base).ok_or(ReadError::InvalidDigit(ch))?;
        value = value
            .checked_mul(i128::from(base))
            .and_then(|v| v.checked_add(i128::from(digit)))
            .ok_or(ReadError::NumberOutOfRange)?;
        digits += 1;
        index += 1;
    }
    if digits == 0 {
        return Err(ReadError::InvalidNumber(collect(token)));
    }
    if negative {
        value = -value;
    }
    if (MIN_FIXNUM..=MAX_FIXNUM).contains(&value) {
        Ok(Word::fixnum(value as i64))
    } else {
        Ok(make_bignum_from_i128(ctx, runtime, value)?.into())
    }
}

/// Parse a base-10 float token into a double-float object.
fn parse_float(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    token: &[char],
    float_format: FloatFormat,
) -> Result<Word, ReadError> {
    let mut normalized = String::with_capacity(token.len());
    let mut marker: Option<char> = None;
    for &ch in token {
        match ch {
            'e' | 'E' => {
                normalized.push('e');
                marker.get_or_insert('e');
            }
            's' | 'S' | 'f' | 'F' | 'd' | 'D' | 'l' | 'L' => {
                normalized.push('e');
                marker = Some(ch);
            }
            _ => normalized.push(ch),
        }
    }
    let target = match marker {
        Some('s' | 'S' | 'f' | 'F') => FloatFormat::SingleFloat,
        Some('d' | 'D' | 'l' | 'L') => FloatFormat::DoubleFloat,
        _ => float_format,
    };
    if target == FloatFormat::SingleFloat {
        return Err(ReadError::UnsupportedFloatFormat(marker.unwrap_or('s')));
    }
    let value: f64 = normalized
        .parse()
        .map_err(|_| ReadError::InvalidNumber(normalized))?;
    Ok(make_double(ctx, runtime, value)?.into())
}

fn collect(token: &[char]) -> String {
    token.iter().collect()
}

/// Parse an integer from `string` within `[start, end)` in `radix` (default 10).
///
/// Returns the integer and the index just past the last digit consumed. This
/// mirrors the Common Lisp `parse-integer` without the `:junk-allowed` option.
///
/// # Errors
/// Returns [`ReadError::InvalidBase`] for an out-of-range radix and
/// [`ReadError::InvalidNumber`] when no digit is present.
pub fn parse_integer(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    string: &str,
    radix: Option<u32>,
    start: Option<usize>,
    end: Option<usize>,
) -> Result<(Word, usize), ReadError> {
    let radix = radix.unwrap_or(10);
    if !(2..=36).contains(&radix) {
        return Err(ReadError::InvalidBase(radix));
    }
    let chars: Vec<char> = string.chars().collect();
    let mut index = start.unwrap_or(0).min(chars.len());
    let end = end.unwrap_or(chars.len()).min(chars.len());

    while index < end && chars[index].is_whitespace() {
        index += 1;
    }
    if index < end && (chars[index] == '+' || chars[index] == '-') {
        index += 1;
    }
    let digits_start = index;
    while index < end && digit_value(chars[index], radix).is_some() {
        index += 1;
    }
    if index == digits_start {
        return Err(ReadError::InvalidNumber(string.to_owned()));
    }
    let integer = parse_integer_chars(ctx, runtime, &chars[..index], radix)?;
    Ok((integer, index))
}
