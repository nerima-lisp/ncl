use ncl_object::Word;
use ncl_printer::{PrintOptions, StringSink, write};

use super::float::{execute_currency_directive, execute_float_directive};
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
        DirectiveKind::W => PrintOptions::new().with_pretty(directive.colon),
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
    if directive.kind == DirectiveKind::W && state.pretty.is_some() {
        execute_pretty_write(state, value, options)?;
        *state.line_start = false;
        return Ok(());
    }
    if matches!(
        directive.kind,
        DirectiveKind::D
            | DirectiveKind::B
            | DirectiveKind::O
            | DirectiveKind::X
            | DirectiveKind::R
    ) && !is_integer(state.ctx, value)
    {
        let mut readable = PrintOptions::new().with_escape(false).with_base(10);
        if directive.kind == DirectiveKind::R {
            readable = readable.with_base(radix_parameter(directive)?);
        }
        let mut fallback = StringSink::new();
        write(state.ctx, state.runtime, value, &mut fallback, &readable)?;
        let rendered = fallback.into_string();
        let width = parameter_width(directive.parameters.first(), directive.kind)?;
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
        for _ in 0..padding {
            state.sink.write_char(pad).map_err(FormatError::from)?;
        }
        state.sink.write_str(&rendered).map_err(FormatError::from)?;
        *state.line_start = false;
        return Ok(());
    }
    let mut rendered = StringSink::new();
    write(state.ctx, state.runtime, value, &mut rendered, &options)?;
    let mut rendered = rendered.into_string();
    if matches!(directive.kind, DirectiveKind::A | DirectiveKind::S) {
        if directive.colon && value == Word::NIL {
            "()".clone_into(&mut rendered);
        }
        return write_character_value(state, directive, &rendered);
    }
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
        parameter_width(directive.parameters.get(1), directive.kind)?
    } else {
        parameter_width(directive.parameters.first(), directive.kind)?
    };
    let pad_index = usize::from(directive.kind == DirectiveKind::R) + 1;
    let padding = width.saturating_sub(rendered.chars().count());
    let pad = directive
        .parameters
        .get(pad_index)
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

fn execute_pretty_write(
    state: &mut ExecutionState<'_>,
    value: Word,
    options: PrintOptions,
) -> Result<(), FormatError> {
    let pretty = state.pretty.as_ref().ok_or(FormatError::InvalidParameter {
        directive: DirectiveKind::W,
    })?;
    pretty
        .borrow_mut()
        .write_object(state.ctx, state.runtime, value, options)?;
    Ok(())
}

fn write_character_value(
    state: &mut ExecutionState<'_>,
    directive: &Directive,
    rendered: &str,
) -> Result<(), FormatError> {
    let mincol = parameter_width(directive.parameters.first(), directive.kind)?;
    let colinc = if matches!(directive.parameters.get(1), Some(Parameter::Character(_))) {
        1
    } else {
        parameter_usize(directive.parameters.get(1), directive.kind)?.unwrap_or(1)
    };
    if colinc == 0 {
        return Err(FormatError::InvalidParameter {
            directive: directive.kind,
        });
    }
    let minpad = parameter_usize(directive.parameters.get(2), directive.kind)?.unwrap_or(0);
    let pad_parameter = directive.parameters.get(3).or_else(|| {
        directive
            .parameters
            .get(1)
            .filter(|parameter| matches!(parameter, Parameter::Character(_)))
    });
    let pad = match pad_parameter {
        None | Some(Parameter::Unsupplied) => ' ',
        Some(Parameter::Character(value)) => *value,
        Some(Parameter::Integer(_) | Parameter::Relative | Parameter::ArgumentCount) => {
            return Err(FormatError::InvalidParameter {
                directive: directive.kind,
            });
        }
    };
    let shortfall = mincol.saturating_sub(rendered.chars().count());
    let width_padding = if shortfall == 0 {
        0
    } else {
        shortfall.div_ceil(colinc) * colinc
    };
    let padding = minpad.max(width_padding);
    if directive.at_sign {
        for _ in 0..padding {
            state.sink.write_char(pad).map_err(FormatError::from)?;
        }
    }
    state.sink.write_str(rendered).map_err(FormatError::from)?;
    if !directive.at_sign {
        for _ in 0..padding {
            state.sink.write_char(pad).map_err(FormatError::from)?;
        }
    }
    *state.line_start = false;
    Ok(())
}

fn group_integer(directive: &Directive, rendered: &str) -> Result<String, FormatError> {
    let comma_index = if directive.kind == DirectiveKind::R { 3 } else { 2 };
    let interval_index = if directive.kind == DirectiveKind::R { 4 } else { 3 };
    let comma = directive
        .parameters
        .get(comma_index)
        .and_then(|parameter| match parameter {
            Parameter::Character(value) => Some(*value),
            Parameter::Integer(_)
            | Parameter::Relative
            | Parameter::ArgumentCount
            | Parameter::Unsupplied => None,
        })
        .unwrap_or(',');
    let interval = parameter_usize(directive.parameters.get(interval_index), directive.kind)?.unwrap_or(3);
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
