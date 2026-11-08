use ncl_object::{ObjectRef, Word, classify_object};
use ncl_printer::{PrintOptions, StringSink, write};

use crate::dispatch::RadixDirective;
use crate::executor::{
    ExecutionState, FormatError, FormatErrorKind, FormatOutput, ResolvedParameter,
};
use crate::model::Directive;

pub(super) fn execute(
    kind: RadixDirective,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    parameters: &[ResolvedParameter],
) -> Result<(), FormatError> {
    match kind {
        RadixDirective::Decimal => integer(directive, state, output, parameters, 10, 0),
        RadixDirective::Binary => integer(directive, state, output, parameters, 2, 0),
        RadixDirective::Octal => integer(directive, state, output, parameters, 8, 0),
        RadixDirective::Hexadecimal => integer(directive, state, output, parameters, 16, 0),
        RadixDirective::Radix => radix(directive, state, output, parameters),
    }
}

fn radix(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    parameters: &[ResolvedParameter],
) -> Result<(), FormatError> {
    if directive.parameters.is_empty() {
        let value = state.take_argument(directive)?;
        if !is_integer(state, value) {
            return Err(wrong_type(directive));
        }
        let decimal = integer_text(state, directive, value, 10)?;
        let words = if directive.at_sign {
            roman(&decimal, directive.colon, directive)?
        } else {
            english(&decimal, directive.colon, directive)?
        };
        return output.write_str(&words);
    }

    let base = match parameters.first() {
        Some(ResolvedParameter::Integer(base)) => *base,
        None | Some(ResolvedParameter::Unsupplied) => 10,
        Some(ResolvedParameter::Character(_)) => {
            return Err(invalid(directive));
        }
    };
    let base = u32::try_from(base)
        .ok()
        .filter(|base| (2..=36).contains(base))
        .ok_or_else(|| invalid(directive))?;
    integer(directive, state, output, parameters, base, 1)
}

fn integer(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    parameters: &[ResolvedParameter],
    base: u32,
    offset: usize,
) -> Result<(), FormatError> {
    if parameters.len() > offset + 4 {
        return Err(invalid(directive));
    }
    let width = integer_parameter(parameters, offset, directive)?.unwrap_or(0);
    let pad = character_parameter(parameters, offset + 1, ' ', directive)?;
    let comma = character_parameter(parameters, offset + 2, ',', directive)?;
    let interval = integer_parameter(parameters, offset + 3, directive)?.unwrap_or(3);
    if interval == 0 {
        return Err(invalid(directive));
    }

    let value = state.take_argument(directive)?;
    if !is_integer(state, value) {
        return format_as_a(directive, state, output, value, width, pad);
    }
    let text = integer_text(state, directive, value, base)?;
    let grouped = if directive.colon {
        group_digits(&text, comma, interval)
    } else {
        text
    };
    output.write_str(&integer_field(&grouped, width, pad, directive.at_sign))
}

fn format_as_a(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    value: Word,
    width: usize,
    pad: char,
) -> Result<(), FormatError> {
    let options = PrintOptions::new()
        .with_escape(false)
        .with_readably(false)
        .with_radix(false);
    if width == 0 {
        return output.print(state, value, &options, directive);
    }
    let mut text = render(state, directive, value, &options)?;
    let padding = repeated(pad, width.saturating_sub(text.chars().count()), directive)?;
    text.push_str(&padding);
    output.write_str(&text)
}

fn integer_field(number: &str, width: usize, pad: char, force_sign: bool) -> String {
    let negative = number.starts_with('-');
    let digits = number.strip_prefix('-').unwrap_or(number);
    let sign = if negative {
        "-"
    } else if force_sign {
        "+"
    } else {
        ""
    };
    let padding = width.saturating_sub(sign.len() + digits.len());
    let mut result = String::new();
    for _ in 0..padding {
        result.push(pad);
    }
    result.push_str(sign);
    result.push_str(digits);
    result
}

fn group_digits(number: &str, separator: char, interval: usize) -> String {
    let (sign, digits) = number
        .strip_prefix('-')
        .map_or(("", number), |digits| ("-", digits));
    let mut result = String::new();
    result.push_str(sign);
    for (index, digit) in digits.chars().enumerate() {
        if index != 0 && (digits.len() - index) % interval == 0 {
            result.push(separator);
        }
        result.push(digit);
    }
    result
}

fn integer_text(
    state: &mut ExecutionState<'_>,
    directive: &Directive,
    value: Word,
    base: u32,
) -> Result<String, FormatError> {
    let options = PrintOptions::new()
        .with_escape(false)
        .with_readably(false)
        .with_radix(false)
        .try_with_base(base)
        .ok_or_else(|| invalid(directive))?;
    Ok(render(state, directive, value, &options)?.to_ascii_uppercase())
}

fn render(
    state: &mut ExecutionState<'_>,
    directive: &Directive,
    value: Word,
    options: &PrintOptions,
) -> Result<String, FormatError> {
    let mut sink = StringSink::new();
    write(state.ctx, state.runtime, value, &mut sink, options)
        .map_err(|error| FormatError::at(directive, FormatErrorKind::Print(error)))?;
    Ok(sink.into_string())
}

fn is_integer(state: &ExecutionState<'_>, value: Word) -> bool {
    matches!(
        classify_object(state.ctx, value),
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_)
    )
}

fn english(decimal: &str, ordinal: bool, directive: &Directive) -> Result<String, FormatError> {
    const SCALE: &[&str] = &[
        "",
        "thousand",
        "million",
        "billion",
        "trillion",
        "quadrillion",
        "quintillion",
        "sextillion",
        "septillion",
        "octillion",
        "nonillion",
        "decillion",
        "undecillion",
        "duodecillion",
        "tredecillion",
        "quattuordecillion",
        "quindecillion",
        "sexdecillion",
        "septendecillion",
        "octodecillion",
        "novemdecillion",
        "vigintillion",
        "unvigintillion",
        "duovigintillion",
        "tresvigintillion",
        "quattuorvigintillion",
        "quinvigintillion",
        "sexvigintillion",
        "septenvigintillion",
        "octovigintillion",
        "novemvigintillion",
        "trigintillion",
    ];
    let (negative, digits) = decimal
        .strip_prefix('-')
        .map_or((false, decimal), |d| (true, d));
    let mut chunks = Vec::new();
    for chunk in digits.as_bytes().rchunks(3) {
        let value = chunk
            .iter()
            .fold(0_u16, |value, digit| value * 10 + u16::from(*digit - b'0'));
        chunks.push(value);
    }
    if chunks.len() > SCALE.len() {
        return Err(invalid(directive));
    }
    let Some(last) = chunks.iter().rposition(|chunk| *chunk != 0) else {
        return Ok(if ordinal { "zeroth" } else { "zero" }.to_owned());
    };
    let mut words = Vec::new();
    if negative {
        words.push("minus".to_owned());
    }
    for index in (0..=last).rev() {
        // check-added-lines: allow(index) rposition returns an index within chunks.
        let value = chunks[index];
        if value == 0 {
            continue;
        }
        let final_chunk = index == last;
        if index == 0 {
            words.extend(under_thousand(value, ordinal && final_chunk, directive)?);
        } else {
            words.extend(under_thousand(value, false, directive)?);
            let scale = SCALE.get(index).ok_or_else(|| invalid(directive))?;
            words.push(if ordinal && final_chunk {
                format!("{scale}th")
            } else {
                (*scale).to_owned()
            });
        }
    }
    Ok(words.join(" "))
}

fn under_thousand(
    value: u16,
    ordinal: bool,
    directive: &Directive,
) -> Result<Vec<String>, FormatError> {
    const SMALL: &[&str] = &[
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
    const ORDINAL: &[&str] = &[
        "zeroth",
        "first",
        "second",
        "third",
        "fourth",
        "fifth",
        "sixth",
        "seventh",
        "eighth",
        "ninth",
        "tenth",
        "eleventh",
        "twelfth",
        "thirteenth",
        "fourteenth",
        "fifteenth",
        "sixteenth",
        "seventeenth",
        "eighteenth",
        "nineteenth",
    ];
    const TENS: &[&str] = &[
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    const TENS_ORDINAL: &[&str] = &[
        "",
        "",
        "twentieth",
        "thirtieth",
        "fortieth",
        "fiftieth",
        "sixtieth",
        "seventieth",
        "eightieth",
        "ninetieth",
    ];
    let hundreds = value / 100;
    let remainder = value % 100;
    let mut words = Vec::new();
    if hundreds != 0 {
        words.push(table_word(SMALL, usize::from(hundreds), directive)?.to_owned());
        words.push(
            if ordinal && remainder == 0 {
                "hundredth"
            } else {
                "hundred"
            }
            .to_owned(),
        );
    }
    if remainder == 0 {
        return Ok(words);
    }
    if remainder < 20 {
        let table = if ordinal { ORDINAL } else { SMALL };
        words.push(table_word(table, usize::from(remainder), directive)?.to_owned());
    } else {
        let tens = usize::from(remainder / 10);
        let units = usize::from(remainder % 10);
        if ordinal && units == 0 {
            words.push(table_word(TENS_ORDINAL, tens, directive)?.to_owned());
        } else {
            let mut word = table_word(TENS, tens, directive)?.to_owned();
            if units != 0 {
                word.push('-');
                let table = if ordinal { ORDINAL } else { SMALL };
                word.push_str(table_word(table, units, directive)?);
            }
            words.push(word);
        }
    }
    Ok(words)
}

fn table_word(
    table: &[&'static str],
    index: usize,
    directive: &Directive,
) -> Result<&'static str, FormatError> {
    table.get(index).copied().ok_or_else(|| invalid(directive))
}

fn roman(decimal: &str, old_style: bool, directive: &Directive) -> Result<String, FormatError> {
    let (negative, digits) = decimal
        .strip_prefix('-')
        .map_or((false, decimal), |d| (true, d));
    let value = digits.parse::<u64>().map_err(|_| invalid(directive))?;
    if value > 100_000 {
        return Err(invalid(directive));
    }
    if value == 0 {
        return Ok("N".to_owned());
    }
    let table: &[(u64, &str)] = if old_style {
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
    if negative {
        result.push('-');
    }
    for (unit, numeral) in table {
        while remaining >= *unit {
            result.push_str(numeral);
            remaining -= *unit;
        }
    }
    Ok(result)
}

fn integer_parameter(
    parameters: &[ResolvedParameter],
    index: usize,
    directive: &Directive,
) -> Result<Option<usize>, FormatError> {
    match parameters.get(index) {
        None | Some(ResolvedParameter::Unsupplied) => Ok(None),
        Some(ResolvedParameter::Integer(value)) => usize::try_from(*value)
            .map(Some)
            .map_err(|_| invalid(directive)),
        Some(ResolvedParameter::Character(_)) => Err(invalid(directive)),
    }
}

fn character_parameter(
    parameters: &[ResolvedParameter],
    index: usize,
    default: char,
    directive: &Directive,
) -> Result<char, FormatError> {
    match parameters.get(index) {
        None | Some(ResolvedParameter::Unsupplied) => Ok(default),
        Some(ResolvedParameter::Character(value)) => Ok(*value),
        Some(ResolvedParameter::Integer(_)) => Err(invalid(directive)),
    }
}

fn repeated(character: char, count: usize, directive: &Directive) -> Result<String, FormatError> {
    let bytes = count
        .checked_mul(character.len_utf8())
        .ok_or_else(|| invalid(directive))?;
    let mut result = String::new();
    result
        .try_reserve_exact(bytes)
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
