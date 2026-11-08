use ncl_object::{
    Bignum, DoubleFloat, ObjectRef, Ratio, Word, bignum_limbs, bignum_sign, classify_object,
    double_value, ratio_denominator, ratio_numerator,
};
use ncl_printer::{PrintOptions, StringSink, write};

use crate::executor::{ExecutionState, FormatError, FormatErrorKind, FormatOutput};
use crate::model::{Directive, Parameter};

const MAX_PRECISION: usize = 10_000;

pub(super) fn number_value(
    state: &ExecutionState<'_>,
    word: Word,
    directive: &Directive,
) -> Result<Option<f64>, FormatError> {
    let object = classify_object(state.ctx, word);
    if let ObjectRef::DoubleFloat(word) = object {
        let object = DoubleFloat::from_word(word);
        let value = double_value(state.ctx, object).map_err(|_| wrong_type(directive))?;
        return Ok(Some(value));
    }
    if matches!(object, ObjectRef::Fixnum(_) | ObjectRef::Bignum(_)) {
        return integer_float(state, word, directive);
    }
    let ObjectRef::Ratio(word) = object else {
        return Ok(None);
    };
    let ratio = Ratio::from_word(word);
    let numerator = ratio_numerator(state.ctx, ratio).map_err(|_| wrong_type(directive))?;
    let denominator = ratio_denominator(state.ctx, ratio).map_err(|_| wrong_type(directive))?;
    let numerator =
        integer_float(state, numerator, directive)?.ok_or_else(|| wrong_type(directive))?;
    let denominator =
        integer_float(state, denominator, directive)?.ok_or_else(|| wrong_type(directive))?;
    if denominator == 0.0 {
        Err(wrong_type(directive))
    } else {
        Ok(Some(numerator / denominator))
    }
}

fn integer_float(
    state: &ExecutionState<'_>,
    word: Word,
    directive: &Directive,
) -> Result<Option<f64>, FormatError> {
    let object = classify_object(state.ctx, word);
    if matches!(object, ObjectRef::Fixnum(_)) {
        return Ok(crate::executor::integer_value(state.ctx, word)
            .and_then(|value| value.to_string().parse::<f64>().ok()));
    }
    let ObjectRef::Bignum(word) = object else {
        return Ok(None);
    };
    let object = Bignum::from_word(word);
    let mut magnitude = 0.0_f64;
    for limb in bignum_limbs(state.ctx, object)
        .map_err(|_| wrong_type(directive))?
        .into_iter()
        .rev()
    {
        magnitude = magnitude.mul_add(4_294_967_296.0, f64::from(limb));
    }
    if bignum_sign(state.ctx, object).map_err(|_| wrong_type(directive))? {
        magnitude = -magnitude;
    }
    Ok(Some(magnitude))
}

pub(super) fn exponential_core(
    value: f64,
    digits: usize,
    scale: i64,
    exponent_width: Option<usize>,
    exponent_char: char,
    directive: &Directive,
) -> Result<String, FormatError> {
    check_precision(digits, directive)?;
    if !value.is_finite() {
        return Ok(value.abs().to_string());
    }
    let scale = i32::try_from(scale).map_err(|_| invalid(directive))?;
    let digits_i32 = i32::try_from(digits).map_err(|_| invalid(directive))?;
    if scale > 0 && scale >= digits_i32.saturating_add(2) {
        return Err(invalid(directive));
    }
    if scale < 0 && scale <= digits_i32.saturating_neg() {
        return Err(invalid(directive));
    }
    let order = if value == 0.0 {
        0
    } else {
        decimal_exponent(value).ok_or_else(|| invalid(directive))?
    };
    let exponent = order
        .checked_add(1)
        .and_then(|e| e.checked_sub(scale))
        .ok_or_else(|| invalid(directive))?;
    let magnitude = value.abs();
    let factor = 10.0_f64.powi(exponent);
    if magnitude != 0.0 && (!factor.is_finite() || factor == 0.0) {
        return Ok(magnitude.to_string());
    }
    let mut mantissa = if magnitude == 0.0 {
        0.0
    } else {
        magnitude / factor
    };
    let fraction = if scale > 0 {
        let scale = usize::try_from(scale).map_err(|_| invalid(directive))?;
        digits
            .checked_add(1)
            .and_then(|value| value.checked_sub(scale))
            .ok_or_else(|| invalid(directive))?
    } else {
        digits
    };
    check_precision(fraction, directive)?;
    let mut exponent = exponent;
    let mut body = format!("{mantissa:.fraction$}");
    let limit = match scale.cmp(&0) {
        std::cmp::Ordering::Greater => 10.0_f64.powi(scale),
        std::cmp::Ordering::Equal => 1.0,
        std::cmp::Ordering::Less => 0.1_f64.powi(scale.saturating_neg()),
    };
    if limit == 0.0 || !limit.is_finite() {
        return Err(invalid(directive));
    }
    if value != 0.0 && mantissa >= limit {
        exponent = exponent.checked_add(1).ok_or_else(|| invalid(directive))?;
        mantissa /= 10.0;
        body = format!("{mantissa:.fraction$}");
    }
    if !body.contains('.') {
        body.push('.');
    }
    let mut integer = body.split('.').next().unwrap_or("0").to_owned();
    let required_integer = if scale > 0 {
        usize::try_from(scale).map_err(|_| invalid(directive))?
    } else {
        1
    };
    if integer.len() < required_integer {
        let zeros = repeated('0', required_integer - integer.len(), directive)?;
        integer = format!("{zeros}{integer}");
        body = format!(
            "{integer}.{}",
            body.split_once('.').map_or("", |(_, fraction)| fraction)
        );
    }
    let negative_exponent = exponent < 0;
    let exponent_digits = exponent.unsigned_abs().to_string();
    let padding = exponent_width
        .unwrap_or(0)
        .saturating_sub(exponent_digits.len());
    let exponent_sign = if negative_exponent { '-' } else { '+' };
    let mut result = String::new();
    result.push_str(&body);
    result.push(exponent_char);
    result.push(exponent_sign);
    result.push_str(&repeated('0', padding, directive)?);
    result.push_str(&exponent_digits);
    Ok(result)
}

pub(super) fn fixed_core(
    value: f64,
    precision: usize,
    minimum_integer: Option<usize>,
    trim: bool,
    directive: &Directive,
) -> Result<String, FormatError> {
    check_precision(precision, directive)?;
    if !value.is_finite() {
        return Ok(value.abs().to_string());
    }
    let text = format!("{:.*}", precision, value.abs());
    let (integer, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let integer_width = minimum_integer.unwrap_or(1);
    let mut integer = if integer.len() < integer_width {
        format!(
            "{}{}",
            repeated('0', integer_width - integer.len(), directive)?,
            integer
        )
    } else {
        integer.to_owned()
    };
    let mut fraction = fraction.to_owned();
    if trim {
        while fraction.ends_with('0') {
            fraction.pop();
        }
        if fraction.is_empty() {
            fraction.push('0');
        }
    }
    integer.push('.');
    integer.push_str(&fraction);
    Ok(integer)
}

pub(super) fn free_float(
    state: &mut ExecutionState<'_>,
    directive: &Directive,
    word: Word,
    value: f64,
) -> Result<String, FormatError> {
    let text = if matches!(classify_object(state.ctx, word), ObjectRef::DoubleFloat(_)) {
        object_text(state, directive, word)?
    } else {
        value.abs().to_string()
    };
    let mut magnitude = text.strip_prefix('-').unwrap_or(&text).to_owned();
    if !magnitude.contains('.') && !magnitude.contains('e') && !magnitude.contains('E') {
        magnitude.push_str(".0");
    }
    Ok(magnitude)
}

fn object_text(
    state: &mut ExecutionState<'_>,
    directive: &Directive,
    word: Word,
) -> Result<String, FormatError> {
    let options = PrintOptions::new()
        .with_escape(false)
        .with_readably(false)
        .with_radix(false);
    let mut sink = StringSink::new();
    write(state.ctx, state.runtime, word, &mut sink, &options)
        .map_err(|error| FormatError::at(directive, FormatErrorKind::Print(error)))?;
    Ok(sink.into_string())
}

// F/E/$ fall through ~wD to ~A for non-numbers, so minimum-width padding follows the value.
pub(super) fn fallback(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    word: Word,
    width: Option<usize>,
) -> Result<(), FormatError> {
    let width = width.unwrap_or(0);
    let options = PrintOptions::new()
        .with_escape(false)
        .with_readably(false)
        .with_radix(false);
    if width == 0 {
        return output.print(state, word, &options, directive);
    }
    let text = object_text(state, directive, word)?;
    let spaces = repeated(' ', width.saturating_sub(text.chars().count()), directive)?;
    let mut padded = text;
    padded.push_str(&spaces);
    output.write_str(&padded)
}

pub(super) fn scaled(value: f64, scale: i64, directive: &Directive) -> Result<f64, FormatError> {
    let scale = i32::try_from(scale).map_err(|_| invalid(directive))?;
    let factor = 10.0_f64.powi(scale);
    let scaled = value * factor;
    if factor.is_finite()
        && factor != 0.0
        && (scaled.is_finite() || scaled == 0.0 || !value.is_finite())
    {
        Ok(scaled)
    } else {
        Err(invalid(directive))
    }
}

pub(super) fn field(
    text: String,
    width: Option<usize>,
    pad: char,
    overflow: Option<char>,
    directive: &Directive,
) -> Result<String, FormatError> {
    let Some(width) = width else { return Ok(text) };
    let length = text.chars().count();
    if length > width {
        if let Some(character) = overflow {
            return repeated(character, width, directive);
        }
        return Ok(text);
    }
    let mut result = repeated(pad, width - length, directive)?;
    result.push_str(&text);
    Ok(result)
}

pub(super) fn decimal_order(value: f64) -> Option<i32> {
    if value == 0.0 {
        return Some(1);
    }
    decimal_exponent(value).and_then(|exponent| exponent.checked_add(1))
}

fn decimal_exponent(value: f64) -> Option<i32> {
    let text = format!("{:e}", value.abs());
    text.rsplit_once('e')?.1.parse().ok()
}

pub(super) fn significant_digits(value: f64) -> usize {
    let mut digits: String = value
        .abs()
        .to_string()
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    while digits.starts_with('0') && digits.len() > 1 {
        digits.remove(0);
    }
    while digits.ends_with('0') && digits.len() > 1 {
        digits.pop();
    }
    digits.len().max(1)
}

pub(super) fn shortest_fraction_digits(value: f64) -> usize {
    let text = value.abs().to_string();
    let mantissa = text
        .split_once('e')
        .or_else(|| text.split_once('E'))
        .map_or(text.as_str(), |(mantissa, _)| mantissa);
    mantissa
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len())
}

pub(super) fn default_fixed_precision(value: f64, width: usize, force_sign: bool) -> usize {
    let integer_width = value.abs().trunc().to_string().len().max(1);
    let reserved = integer_width
        .saturating_add(sign(value, force_sign).len())
        .saturating_add(1);
    let precision = width.saturating_sub(reserved);
    if precision == 0 && value.abs() < 1.0 && width > sign(value, force_sign).len() + 1 {
        1
    } else {
        precision
    }
}

pub(super) fn default_exponential_precision(value: f64, scale: i64) -> usize {
    let significant = i64::try_from(significant_digits(value)).unwrap_or(i64::MAX);
    let precision = if scale > 0 {
        significant.saturating_sub(1).max(scale.saturating_sub(1))
    } else {
        significant.saturating_sub(scale)
    }
    .max(1);
    usize::try_from(precision).unwrap_or(MAX_PRECISION)
}

pub(super) fn signed(text: &str, value: f64, force: bool) -> String {
    format!("{}{text}", sign(value, force))
}

pub(super) const fn sign(value: f64, force: bool) -> &'static str {
    if value.is_sign_negative() {
        "-"
    } else if force {
        "+"
    } else {
        ""
    }
}

pub(super) fn integer_parameter(
    parameters: &[Parameter],
    index: usize,
    directive: &Directive,
) -> Result<Option<usize>, FormatError> {
    match parameters.get(index) {
        None | Some(Parameter::Unsupplied) => Ok(None),
        Some(Parameter::Integer(value)) => usize::try_from(*value)
            .map(Some)
            .map_err(|_| invalid(directive)),
        Some(Parameter::Character(_) | Parameter::Relative | Parameter::Remaining) => {
            Err(invalid(directive))
        }
    }
}

pub(super) fn signed_parameter(
    parameters: &[Parameter],
    index: usize,
    default: i64,
    directive: &Directive,
) -> Result<i64, FormatError> {
    match parameters.get(index) {
        None | Some(Parameter::Unsupplied) => Ok(default),
        Some(Parameter::Integer(value)) => Ok(*value),
        Some(Parameter::Character(_) | Parameter::Relative | Parameter::Remaining) => {
            Err(invalid(directive))
        }
    }
}

pub(super) fn character_parameter(
    parameters: &[Parameter],
    index: usize,
    default: char,
    directive: &Directive,
) -> Result<char, FormatError> {
    match parameters.get(index) {
        None | Some(Parameter::Unsupplied) => Ok(default),
        Some(Parameter::Character(value)) => Ok(*value),
        Some(Parameter::Integer(_) | Parameter::Relative | Parameter::Remaining) => {
            Err(invalid(directive))
        }
    }
}

pub(super) fn optional_character(
    parameters: &[Parameter],
    index: usize,
    directive: &Directive,
) -> Result<Option<char>, FormatError> {
    match parameters.get(index) {
        None | Some(Parameter::Unsupplied) => Ok(None),
        Some(Parameter::Character(value)) => Ok(Some(*value)),
        Some(Parameter::Integer(_) | Parameter::Relative | Parameter::Remaining) => {
            Err(invalid(directive))
        }
    }
}

pub(super) const fn check_precision(
    value: usize,
    directive: &Directive,
) -> Result<(), FormatError> {
    if value > MAX_PRECISION {
        Err(invalid(directive))
    } else {
        Ok(())
    }
}

pub(super) fn repeated(
    character: char,
    count: usize,
    directive: &Directive,
) -> Result<String, FormatError> {
    let size = count
        .checked_mul(character.len_utf8())
        .ok_or_else(|| invalid(directive))?;
    let mut result = String::new();
    result
        .try_reserve_exact(size)
        .map_err(|_| invalid(directive))?;
    for _ in 0..count {
        result.push(character);
    }
    Ok(result)
}

pub(super) const fn invalid(directive: &Directive) -> FormatError {
    FormatError::at(directive, FormatErrorKind::InvalidParameter)
}

const fn wrong_type(directive: &Directive) -> FormatError {
    FormatError::at(directive, FormatErrorKind::WrongArgumentType)
}
