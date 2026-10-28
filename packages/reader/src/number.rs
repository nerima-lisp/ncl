//! Number token parsing: potential-number recognition and construction.

use ncl_object::{
    HeapRootedWord, Runtime, ThreadContext, Word, make_bignum_from_limbs, make_double, make_ratio,
};

use crate::error::ReadError;
use crate::reader::FloatFormat;

/// The largest fixnum value (`most-positive-fixnum`).
const MAX_FIXNUM: u64 = (1_u64 << 62) - 1;
/// The smallest fixnum value (`most-negative-fixnum`).
const MIN_FIXNUM_MAGNITUDE: u64 = 1_u64 << 62;

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
    let has_sign = matches!(token.first(), Some('+' | '-'));

    if has_sign {
        index += 1;
    }

    let mut digits_before = 0_usize;
    while token
        .get(index)
        .and_then(|ch| digit_value(*ch, base))
        .is_some()
    {
        digits_before += 1;
        index += 1;
    }

    if token.get(index) == Some(&'/') {
        if digits_before == 0 {
            return None;
        }
        index += 1;
        if matches!(token.get(index), Some('+' | '-')) {
            index += 1;
        }
        let mut digits_after = 0_usize;
        while token
            .get(index)
            .and_then(|ch| digit_value(*ch, base))
            .is_some()
        {
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
    if token.get(index) == Some(&'.') {
        has_dot = true;
        index += 1;
        while token
            .get(index)
            .and_then(|ch| digit_value(*ch, base))
            .is_some()
        {
            has_digits_after_dot = true;
            index += 1;
        }
    }

    let mut has_exponent = false;
    if base == 10
        && (digits_before > 0 || has_dot || has_sign)
        && let Some(marker) = token.get(index).copied()
        && matches!(
            marker,
            'e' | 'E' | 's' | 'S' | 'f' | 'F' | 'd' | 'D' | 'l' | 'L'
        )
    {
        has_exponent = true;
        index += 1;
        if matches!(token.get(index), Some('+' | '-')) {
            index += 1;
        }
        let mut exp_digits = 0_usize;
        while token
            .get(index)
            .and_then(|ch| digit_value(*ch, 10))
            .is_some()
        {
            exp_digits += 1;
            index += 1;
        }
        if exp_digits == 0 {
            return (index == n && (digits_before > 0 || has_dot)).then_some(NumberShape::Float);
        }
        if index != n {
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
            let numerator = parse_integer_chars(
                ctx,
                runtime,
                token
                    .get(..slash)
                    .ok_or_else(|| ReadError::InvalidNumber(collect(token)))?,
                base,
            )?;
            let numerator = HeapRootedWord::new(ctx, numerator)?;
            let denominator = parse_integer_chars(
                ctx,
                runtime,
                token
                    .get(slash + 1..)
                    .ok_or_else(|| ReadError::InvalidNumber(collect(token)))?,
                base,
            )?;
            Ok(Some(
                make_ratio(ctx, runtime, numerator.get(), denominator)?.into(),
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
    let mut limbs = vec![0_u32];
    let mut digits = 0_usize;
    while let Some(&ch) = token.get(index) {
        if ch == '.' {
            break;
        }
        let digit = digit_value(ch, base).ok_or(ReadError::InvalidDigit(ch))?;
        let mut carry = u64::from(digit);
        for limb in &mut limbs {
            let value = u64::from(*limb) * u64::from(base) + carry;
            *limb = u32::try_from(value & u64::from(u32::MAX))
                .map_err(|_| ReadError::NumberOutOfRange)?;
            carry = value >> 32;
        }
        if carry != 0 {
            limbs.push(u32::try_from(carry).map_err(|_| ReadError::NumberOutOfRange)?);
        }
        digits += 1;
        index += 1;
    }
    if digits == 0 {
        return Err(ReadError::InvalidNumber(collect(token)));
    }
    while limbs.len() > 1 && limbs.last() == Some(&0) {
        limbs.pop();
    }
    if limbs.len() <= 2 {
        let magnitude = u64::from(limbs.first().copied().unwrap_or(0))
            | (u64::from(*limbs.get(1).unwrap_or(&0)) << 32);
        let fixnum_limit = if negative {
            MIN_FIXNUM_MAGNITUDE
        } else {
            MAX_FIXNUM
        };
        if magnitude <= fixnum_limit {
            let value = i64::try_from(magnitude).map_err(|_| ReadError::NumberOutOfRange)?;
            return Ok(Word::fixnum(if negative { -value } else { value }));
        }
    }
    Ok(make_bignum_from_limbs(ctx, runtime, negative, &limbs)?.into())
}

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
    let target = match marker.map(|marker| marker.to_ascii_lowercase()) {
        Some('s' | 'f') => FloatFormat::SingleFloat,
        Some('d' | 'l') => FloatFormat::DoubleFloat,
        _ => float_format,
    };
    let value: f64 = normalized
        .parse()
        .map_err(|_| ReadError::InvalidNumber(normalized))?;

    // NCL stores every selected float format in its single DoubleFloat object representation.
    match target {
        FloatFormat::SingleFloat | FloatFormat::DoubleFloat => {
            Ok(make_double(ctx, runtime, value)?.into())
        }
    }
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

    while chars.get(index).is_some_and(|ch| ch.is_whitespace()) {
        index += 1;
    }
    if matches!(chars.get(index), Some('+' | '-')) {
        index += 1;
    }
    let digits_start = index;
    while index < end
        && chars
            .get(index)
            .and_then(|ch| digit_value(*ch, radix))
            .is_some()
    {
        index += 1;
    }
    if index == digits_start {
        return Err(ReadError::InvalidNumber(string.to_owned()));
    }
    let integer = parse_integer_chars(
        ctx,
        runtime,
        chars
            .get(..index)
            .ok_or_else(|| ReadError::InvalidNumber(string.to_owned()))?,
        radix,
    )?;
    Ok((integer, index))
}
