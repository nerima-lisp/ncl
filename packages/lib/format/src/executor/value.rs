use ncl_object::{ObjectRef, Word, classify_object, double_value};
use ncl_printer::{PrintError, PrintOptions, StringSink, write};

use super::parameters::{is_integer, parameter_i64, parameter_usize, parameter_width};
use super::{ExecutionState, FormatError, next_argument};
use crate::{Directive, DirectiveKind, Parameter};

pub(super) fn execute_value_kind(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let options = match directive.kind {
        DirectiveKind::A => PrintOptions::new().with_escape(false),
        DirectiveKind::S => PrintOptions::new().with_escape(true).with_readably(true),
        DirectiveKind::D => PrintOptions::new().with_base(10),
        DirectiveKind::B => PrintOptions::new().with_base(2),
        DirectiveKind::O => PrintOptions::new().with_base(8),
        DirectiveKind::X => PrintOptions::new().with_base(16),
        DirectiveKind::R if directive.parameters.is_empty() => {
            return execute_unparameterized_radix(directive, state);
        }
        DirectiveKind::R => PrintOptions::new().with_base(radix_parameter(directive)?),
        DirectiveKind::W => PrintOptions::new().with_readably(true),
        DirectiveKind::F | DirectiveKind::E | DirectiveKind::G => {
            return execute_float_directive(directive, state);
        }
        DirectiveKind::Dollar => return execute_currency_directive(directive, state),
        // check-added-lines: allow(wildcard) non-value directives
        _ => {
            return Err(FormatError::InvalidParameter {
                directive: directive.kind,
            });
        }
    };
    execute_value_directive(directive, state, options)
}

fn execute_unparameterized_radix(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let value = next_argument(directive, state)?;
    if !is_integer(state.ctx, value) {
        return Err(FormatError::NonInteger {
            directive: directive.kind,
        });
    }
    let mut rendered = StringSink::new();
    write(
        state.ctx,
        state.runtime,
        value,
        &mut rendered,
        &PrintOptions::new().with_base(10),
    )?;
    let number =
        rendered
            .into_string()
            .parse::<i128>()
            .map_err(|_| FormatError::InvalidParameter {
                directive: directive.kind,
            })?;
    let rendered = if directive.at_sign {
        roman(number, directive.colon)
    } else if directive.colon {
        ordinal(number)
    } else {
        cardinal(number)
    };
    write_padded(state, directive, &rendered)
}

fn cardinal(value: i128) -> String {
    if value < 0 {
        return format!("minus {}", cardinal(-value));
    }
    if value == 0 {
        return "zero".to_owned();
    }
    let mut result = String::new();
    let mut remaining = value;
    for (scale, name) in [
        (1_000_000_000_000_i128, "trillion"),
        (1_000_000_000_i128, "billion"),
        (1_000_000_i128, "million"),
        (1_000_i128, "thousand"),
    ] {
        if remaining >= scale {
            append_words(&mut result, &under_thousand(remaining / scale));
            result.push(' ');
            result.push_str(name);
            remaining %= scale;
            if remaining != 0 {
                result.push(' ');
            }
        }
    }
    if remaining != 0 {
        append_words(&mut result, &under_thousand(remaining));
    }
    result
}

pub(super) fn append_words(result: &mut String, words: &str) {
    if !result.is_empty() && !result.ends_with(' ') {
        result.push(' ');
    }
    result.push_str(words);
}

fn under_thousand(value: i128) -> String {
    const ONES: [&str; 20] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 10] = [
        "zero", "ten", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    if value < 20 {
        let index = usize::try_from(value).unwrap_or(0);
        return ONES.get(index).copied().unwrap_or("zero").to_owned();
    }
    if value < 100 {
        let tens = value / 10;
        let ones = value % 10;
        return if ones == 0 {
            let index = usize::try_from(tens).unwrap_or(0);
            TENS.get(index).copied().unwrap_or("zero").to_owned()
        } else {
            let tens_index = usize::try_from(tens).unwrap_or(0);
            let ones_index = usize::try_from(ones).unwrap_or(0);
            format!(
                "{}-{}",
                TENS.get(tens_index).copied().unwrap_or("zero"),
                ONES.get(ones_index).copied().unwrap_or("zero")
            )
        };
    }
    let hundreds = value / 100;
    let rest = value % 100;
    let index = usize::try_from(hundreds).unwrap_or(0);
    if rest == 0 {
        format!("{} hundred", ONES.get(index).copied().unwrap_or("zero"))
    } else {
        format!(
            "{} hundred {}",
            ONES.get(index).copied().unwrap_or("zero"),
            under_thousand(rest)
        )
    }
}

fn ordinal(value: i128) -> String {
    if value < 0 {
        return format!("minus {}", ordinal(-value));
    }
    let cardinal = cardinal(value);
    if let Some(prefix) = cardinal.strip_suffix("one") {
        return format!("{prefix}first");
    }
    if let Some(prefix) = cardinal.strip_suffix("two") {
        return format!("{prefix}second");
    }
    if let Some(prefix) = cardinal.strip_suffix("three") {
        return format!("{prefix}third");
    }
    if let Some(prefix) = cardinal.strip_suffix("five") {
        return format!("{prefix}fifth");
    }
    if let Some(prefix) = cardinal.strip_suffix("eight") {
        return format!("{prefix}eighth");
    }
    if let Some(prefix) = cardinal.strip_suffix("nine") {
        return format!("{prefix}ninth");
    }
    if let Some(prefix) = cardinal.strip_suffix("twelve") {
        return format!("{prefix}twelfth");
    }
    if let Some(prefix) = cardinal.strip_suffix("y") {
        return format!("{prefix}ieth");
    }
    format!("{cardinal}th")
}

fn roman(value: i128, old: bool) -> String {
    if value == 0 {
        return "N".to_owned();
    }
    if value < 0 {
        return format!("-{}", roman(-value, old));
    }
    let symbols: &[(i128, &str)] = if old {
        &[
            (1000, "M"),
            (500, "D"),
            (100, "C"),
            (50, "L"),
            (10, "X"),
            (5, "V"),
            (1, "I"),
        ]
    } else {
        &[
            (1000, "M"),
            (900, "CM"),
            (500, "D"),
            (400, "CD"),
            (100, "C"),
            (90, "XC"),
            (50, "L"),
            (40, "XL"),
            (10, "X"),
            (9, "IX"),
            (5, "V"),
            (4, "IV"),
            (1, "I"),
        ]
    };
    let mut remaining = value;
    let mut result = String::new();
    for (unit, symbol) in symbols {
        while remaining >= *unit {
            result.push_str(symbol);
            remaining -= *unit;
        }
    }
    result
}

fn execute_float_directive(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let object = next_argument(directive, state)?;
    let ObjectRef::DoubleFloat(number) = classify_object(state.ctx, object) else {
        return Err(FormatError::InvalidParameter {
            directive: directive.kind,
        });
    };
    let mut value = double_value(state.ctx, ncl_object::DoubleFloat::from_word(number))
        .map_err(PrintError::from)?;
    let scale_index = if directive.kind == DirectiveKind::E || directive.kind == DirectiveKind::G {
        3
    } else {
        2
    };
    if let Some(scale) = parameter_i64(directive.parameters.get(scale_index)) {
        value *= 10_f64.powi(
            i32::try_from(scale).map_err(|_| FormatError::InvalidParameter {
                directive: directive.kind,
            })?,
        );
    }
    let mut rendered = render_float(state, value, object, directive)?;
    if directive.at_sign && !rendered.starts_with('-') {
        rendered.insert(0, '+');
    }
    let width = parameter_usize(directive.parameters.first(), directive.kind)?;
    let overflow_index = if directive.kind == DirectiveKind::F {
        3
    } else {
        4
    };
    let pad_index = if directive.kind == DirectiveKind::F {
        4
    } else {
        5
    };
    let overflow = parameter_char(directive, overflow_index)?;
    let pad = parameter_char(directive, pad_index)?.unwrap_or(' ');
    if let Some(width) = width {
        if rendered.chars().count() > width {
            if let Some(overflow) = overflow {
                rendered = std::iter::repeat_n(overflow, width).collect();
            }
        } else {
            rendered = format!(
                "{}{}",
                pad.to_string().repeat(width - rendered.chars().count()),
                rendered
            );
        }
    }
    state.sink.write_str(&rendered).map_err(FormatError::from)?;
    *state.line_start = false;
    Ok(())
}

fn execute_currency_directive(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let value = next_argument(directive, state)?;
    let ObjectRef::DoubleFloat(number) = classify_object(state.ctx, value) else {
        return Err(FormatError::InvalidParameter {
            directive: directive.kind,
        });
    };
    let value = double_value(state.ctx, ncl_object::DoubleFloat::from_word(number))
        .map_err(PrintError::from)?;
    let digits = parameter_usize(directive.parameters.first(), directive.kind)?.unwrap_or(2);
    let minimum_units = parameter_usize(directive.parameters.get(1), directive.kind)?.unwrap_or(1);
    let width = parameter_usize(directive.parameters.get(2), directive.kind)?.unwrap_or(0);
    let pad = parameter_char(directive, 3)?.unwrap_or(' ');
    let magnitude = format!("{value:.digits$}");
    let unsigned_sign = if directive.at_sign {
        ("+", magnitude.as_str())
    } else {
        ("", magnitude.as_str())
    };
    let (sign, unsigned) = magnitude
        .strip_prefix('-')
        .map_or(unsigned_sign, |unsigned| ("-", unsigned));
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let whole = format!("{whole:0>minimum_units$}");
    let rendered = format!("{sign}{whole}.{fraction}");
    let padding = width.saturating_sub(rendered.chars().count());
    if directive.colon {
        state.sink.write_str(sign).map_err(FormatError::from)?;
        state
            .sink
            .write_str(&format!(
                "{}{}.{fraction}",
                pad.to_string().repeat(padding),
                whole
            ))
            .map_err(FormatError::from)?;
    } else {
        state
            .sink
            .write_str(&format!("{}{rendered}", pad.to_string().repeat(padding)))
            .map_err(FormatError::from)?;
    }
    *state.line_start = false;
    Ok(())
}

fn render_float(
    state: &mut ExecutionState<'_>,
    value: f64,
    object: Word,
    directive: &Directive,
) -> Result<String, FormatError> {
    let Some(digits) = parameter_usize(directive.parameters.get(1), directive.kind)? else {
        let mut sink = StringSink::new();
        write(
            state.ctx,
            state.runtime,
            object,
            &mut sink,
            &PrintOptions::new(),
        )?;
        return Ok(sink.into_string());
    };
    if directive.kind == DirectiveKind::E {
        let mut rendered = format!("{value:.digits$e}");
        if let Some(exponent_width) = parameter_usize(directive.parameters.get(2), directive.kind)?
            && let Some((mantissa, exponent)) = rendered.split_once('e')
        {
            let sign = exponent.strip_prefix('+').map_or("", |_| "+");
            let digits = exponent.trim_start_matches(['+', '-']);
            let exponent_sign = if exponent.starts_with('-') { "-" } else { sign };
            rendered = format!("{mantissa}e{exponent_sign}{digits:0>exponent_width$}");
        }
        Ok(rendered)
    } else if directive.kind == DirectiveKind::G {
        let exponent = value.abs() >= 1_000_000_f64 || (value != 0.0 && value.abs() < 0.0001);
        if exponent {
            let mut exponential = format!("{value:.digits$e}");
            if let Some(exponent_char) = parameter_char(directive, 6)? {
                exponential = exponential.replace('e', &exponent_char.to_string());
            }
            Ok(exponential)
        } else {
            Ok(fixed_float(value, digits))
        }
    } else {
        Ok(fixed_float(value, digits))
    }
}

fn parameter_char(directive: &Directive, index: usize) -> Result<Option<char>, FormatError> {
    match directive.parameters.get(index) {
        None | Some(Parameter::Unsupplied) => Ok(None),
        Some(Parameter::Character(value)) => Ok(Some(*value)),
        Some(Parameter::Integer(_) | Parameter::Relative | Parameter::ArgumentCount) => {
            Err(FormatError::InvalidParameter {
                directive: directive.kind,
            })
        }
    }
}

fn fixed_float(value: f64, digits: usize) -> String {
    format!("{value:.digits$}")
}

fn write_padded(
    state: &mut ExecutionState<'_>,
    directive: &Directive,
    rendered: &str,
) -> Result<(), FormatError> {
    let width = parameter_width(directive.parameters.first(), directive.kind)?;
    let pad = directive
        .parameters
        .get(4)
        .and_then(|parameter| {
            if let Parameter::Character(value) = parameter {
                Some(*value)
            } else {
                None
            }
        })
        .unwrap_or(' ');
    let padding = width.saturating_sub(rendered.chars().count());
    for _ in 0..padding {
        state.sink.write_char(pad).map_err(FormatError::from)?;
    }
    state.sink.write_str(rendered).map_err(FormatError::from)?;
    *state.line_start = false;
    Ok(())
}

fn execute_value_directive(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    options: PrintOptions,
) -> Result<(), FormatError> {
    let value = next_argument(directive, state)?;
    execute_value_with_argument(directive, state, options, value)
}

fn execute_value_with_argument(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    options: PrintOptions,
    value: Word,
) -> Result<(), FormatError> {
    if matches!(
        directive.kind,
        DirectiveKind::D
            | DirectiveKind::B
            | DirectiveKind::O
            | DirectiveKind::X
            | DirectiveKind::R
    ) && !is_integer(state.ctx, value)
    {
        return Err(FormatError::NonInteger {
            directive: directive.kind,
        });
    }
    let mut rendered = StringSink::new();
    write(state.ctx, state.runtime, value, &mut rendered, &options)?;
    let mut rendered = rendered.into_string();
    if matches!(
        directive.kind,
        DirectiveKind::D
            | DirectiveKind::B
            | DirectiveKind::O
            | DirectiveKind::X
            | DirectiveKind::R
    ) {
        if directive.colon {
            rendered = group_integer(directive, &rendered)?;
        }
        if directive.at_sign && !rendered.starts_with('-') {
            rendered.insert(0, '+');
        }
    }
    let width = if directive.kind == DirectiveKind::R {
        0
    } else {
        parameter_width(directive.parameters.first(), directive.kind)?
    };
    let padding = width.saturating_sub(rendered.chars().count());
    let pad = directive
        .parameters
        .get(1)
        .and_then(|parameter| match parameter {
            Parameter::Character(value) => Some(*value),
            Parameter::Integer(_)
            | Parameter::Relative
            | Parameter::ArgumentCount
            | Parameter::Unsupplied => None,
        })
        .unwrap_or(' ');
    let right_pad = matches!(
        directive.kind,
        DirectiveKind::A | DirectiveKind::S | DirectiveKind::W
    ) && !directive.at_sign;
    if !right_pad {
        for _ in 0..padding {
            state.sink.write_char(pad).map_err(FormatError::from)?;
        }
    }
    state.sink.write_str(&rendered).map_err(FormatError::from)?;
    if right_pad {
        for _ in 0..padding {
            state.sink.write_char(pad).map_err(FormatError::from)?;
        }
    }
    *state.line_start = false;
    Ok(())
}

fn group_integer(directive: &Directive, rendered: &str) -> Result<String, FormatError> {
    let comma = directive
        .parameters
        .get(2)
        .and_then(|parameter| match parameter {
            Parameter::Character(value) => Some(*value),
            Parameter::Integer(_)
            | Parameter::Relative
            | Parameter::ArgumentCount
            | Parameter::Unsupplied => None,
        })
        .unwrap_or(',');
    let interval = parameter_usize(directive.parameters.get(3), directive.kind)?.unwrap_or(3);
    if interval == 0 {
        return Err(FormatError::InvalidParameter {
            directive: directive.kind,
        });
    }
    let (sign, digits) = rendered
        .strip_prefix('-')
        .map_or(("", rendered), |digits| ("-", digits));
    let digit_chars: Vec<char> = digits.chars().collect();
    let mut grouped = String::with_capacity(rendered.len() + digit_chars.len() / interval);
    grouped.push_str(sign);
    let first = digit_chars.len() % interval;
    if first != 0 {
        grouped.extend(digit_chars.iter().take(first));
    }
    for (index, digit) in digit_chars.iter().skip(first).enumerate() {
        if index % interval == 0 && (first != 0 || index != 0) {
            grouped.push(comma);
        }
        grouped.push(*digit);
    }
    Ok(grouped)
}

fn radix_parameter(directive: &Directive) -> Result<u32, FormatError> {
    let value = parameter_i64(directive.parameters.first()).unwrap_or(10);
    if !(2..=36).contains(&value) {
        return Err(FormatError::InvalidParameter {
            directive: directive.kind,
        });
    }
    u32::try_from(value).map_err(|_| FormatError::InvalidParameter {
        directive: directive.kind,
    })
}
