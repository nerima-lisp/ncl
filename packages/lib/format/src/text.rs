//! Character, object, plural, and line-oriented directives.

use ncl_object::{ObjectRef, Word, classify_object};
use ncl_printer::PrintOptions;

use crate::dispatch::{LineDirective, TextDirective, ValueDirective};
use crate::executor::{ExecutionState, FormatError, FormatErrorKind, FormatOutput};
use crate::model::{Directive, Parameter};

pub fn execute(
    kind: TextDirective,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    match kind {
        TextDirective::Value(kind) => execute_value(kind, directive, state, output),
        TextDirective::Character => execute_character(directive, state, output),
        TextDirective::Plural => execute_plural(directive, state, output),
        TextDirective::Line(kind) => execute_line(kind, directive, state, output),
    }
}

fn execute_value(
    kind: ValueDirective,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    let parameters = state.parameters(directive)?;
    let value = state.take_argument(directive)?;
    let options = match kind {
        ValueDirective::Aesthetic => PrintOptions::new().with_escape(false),
        ValueDirective::Standard => PrintOptions::new().with_escape(true).with_readably(true),
        ValueDirective::Write => {
            let mut options = PrintOptions::from_specials(state.ctx, state.runtime);
            if directive.colon {
                options = options.with_pretty(true);
            }
            if directive.at_sign {
                options = options.with_level(None).with_length(None);
            }
            options
        }
    };
    let rendered = FormatOutput::render(state, value, &options, directive)?;
    let rendered = if directive.colon && value == Word::NIL {
        "()".to_owned()
    } else {
        rendered
    };
    if matches!(kind, ValueDirective::Write) && !parameters.is_empty() {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidParameter,
        ));
    }
    let mincol = parameter(&parameters, 0, 0, directive)?;
    let colinc = parameter(&parameters, 1, 1, directive)?;
    let minpad = parameter(&parameters, 2, 0, directive)?;
    let padchar = match parameters.get(3) {
        None | Some(Parameter::Unsupplied) => ' ',
        Some(Parameter::Character(character)) => *character,
        Some(_) => {
            return Err(FormatError::at(
                directive,
                FormatErrorKind::InvalidParameter,
            ));
        }
    };
    if mincol < 0 || minpad < 0 || colinc <= 0 {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidParameter,
        ));
    }
    let length = i64::try_from(rendered.chars().count())
        .map_err(|_| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
    let required_width = length.saturating_add(minpad);
    let width = if required_width <= mincol {
        mincol
    } else {
        let extra = required_width.saturating_sub(mincol);
        mincol.saturating_add(extra.saturating_add(colinc - 1) / colinc * colinc)
    };
    let padding = width.saturating_sub(length);
    let padding = usize::try_from(padding)
        .map_err(|_| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
    let mut field = String::with_capacity(rendered.len().saturating_add(padding));
    if directive.at_sign {
        field.extend(std::iter::repeat_n(padchar, padding));
    }
    field.push_str(&rendered);
    if !directive.at_sign {
        field.extend(std::iter::repeat_n(padchar, padding));
    }
    output.write_str(&field)
}

fn execute_character(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    let parameters = state.parameters(directive)?;
    let value = state.take_argument(directive)?;
    if !parameters.is_empty() {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidParameter,
        ));
    }
    let ObjectRef::Character(codepoint) = classify_object(state.ctx, value) else {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::WrongArgumentType,
        ));
    };
    let character = char::from_u32(codepoint)
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::WrongArgumentType))?;
    let name = character_name(character);
    let rendered = if directive.colon {
        name
    } else if directive.at_sign {
        format!("#\\{name}")
    } else {
        character.to_string()
    };
    output.write_str(&rendered)
}

fn execute_plural(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    let parameters = state.parameters(directive)?;
    if !parameters.is_empty() {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidParameter,
        ));
    }
    let singular = if directive.colon {
        state
            .previous_is_one
            .ok_or_else(|| FormatError::at(directive, FormatErrorKind::MissingArgument))?
    } else {
        state.take_argument(directive)? == Word::fixnum(1)
    };
    if directive.colon {
        state.index = state.index.saturating_sub(1);
    }
    let text = if directive.at_sign {
        if singular { "y" } else { "ies" }
    } else if singular {
        ""
    } else {
        "s"
    };
    output.write_str(text)
}

fn execute_line(
    kind: LineDirective,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    let parameters = state.parameters(directive)?;
    let count = match parameters.first() {
        None | Some(Parameter::Unsupplied) => 1,
        Some(Parameter::Integer(value)) if *value >= 0 => usize::try_from(*value)
            .map_err(|_| FormatError::at(directive, FormatErrorKind::InvalidParameter))?,
        Some(_) => {
            return Err(FormatError::at(
                directive,
                FormatErrorKind::InvalidParameter,
            ));
        }
    };
    match kind {
        LineDirective::Percent | LineDirective::Bar => {
            let character = if matches!(kind, LineDirective::Percent) {
                '\n'
            } else {
                '\u{c}'
            };
            for _ in 0..count {
                output.write_char(character)?;
            }
        }
        LineDirective::Ampersand => {
            if count > 0 && !output.line_start {
                output.write_char('\n')?;
            }
            for _ in 1..count {
                output.write_char('\n')?;
            }
        }
        LineDirective::Tilde => {
            for _ in 0..count {
                output.write_char('~')?;
            }
        }
        LineDirective::Newline => {
            if directive.at_sign {
                output.write_char('\n')?;
            }
        }
    }
    Ok(())
}

fn parameter(
    parameters: &[Parameter],
    index: usize,
    default: i64,
    directive: &Directive,
) -> Result<i64, FormatError> {
    match parameters.get(index) {
        None | Some(Parameter::Unsupplied) => Ok(default),
        Some(Parameter::Integer(value)) => Ok(*value),
        Some(_) => Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidParameter,
        )),
    }
}

fn character_name(character: char) -> String {
    [
        ('\u{0000}', "Null"),
        ('\u{0008}', "Backspace"),
        ('\t', "Tab"),
        ('\n', "Newline"),
        ('\u{000c}', "Page"),
        ('\r', "Return"),
        (' ', "Space"),
        ('\u{007f}', "Rubout"),
    ]
    .into_iter()
    .find_map(|(known, name)| (known == character).then_some(name))
    .map_or_else(|| character.to_string(), String::from)
}
