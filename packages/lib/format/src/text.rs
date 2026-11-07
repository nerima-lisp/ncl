//! Character, object, plural, and line-oriented directives.

use ncl_object::{ObjectRef, Word, classify_object};
use ncl_printer::PrintOptions;

use crate::executor::{ExecutionState, FormatError, FormatErrorKind, FormatOutput};
use crate::model::{Directive, DirectiveKind, Parameter};

pub(crate) fn execute(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    match directive.kind {
        DirectiveKind::A | DirectiveKind::S | DirectiveKind::W | DirectiveKind::C => {
            execute_value(directive, state, output)
        }
        DirectiveKind::Plural => execute_plural(directive, state, output),
        DirectiveKind::Percent
        | DirectiveKind::Ampersand
        | DirectiveKind::Bar
        | DirectiveKind::Tilde
        | DirectiveKind::Newline => execute_line(directive, state, output),
        _ => Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        )),
    }
}

fn execute_value(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    let parameters = state.parameters(directive)?;
    let value = state.take_argument(directive)?;
    if directive.kind == DirectiveKind::C {
        if !parameters.is_empty() {
            return Err(FormatError::at(
                directive,
                FormatErrorKind::InvalidParameter,
            ));
        }
        let codepoint = match classify_object(state.ctx, value) {
            ObjectRef::Character(codepoint) => codepoint,
            _ => {
                return Err(FormatError::at(
                    directive,
                    FormatErrorKind::WrongArgumentType,
                ));
            }
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
        return output.write_str(&rendered);
    }

    let options = match directive.kind {
        DirectiveKind::A => PrintOptions::new().with_escape(false),
        DirectiveKind::S => PrintOptions::new().with_escape(true).with_readably(true),
        DirectiveKind::W => {
            let mut options = PrintOptions::from_specials(state.ctx, state.runtime);
            if directive.colon {
                options = options.with_pretty(true);
            }
            if directive.at_sign {
                options = options.with_level(None).with_length(None);
            }
            options
        }
        _ => {
            return Err(FormatError::at(
                directive,
                FormatErrorKind::InvalidDirectiveUse,
            ));
        }
    };
    let rendered = output.render(state, value, &options, directive)?;
    let rendered = if directive.colon && value == Word::NIL {
        "()".to_owned()
    } else {
        rendered
    };
    if directive.kind == DirectiveKind::W && !parameters.is_empty() {
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
    let singular = if directive.at_sign && !directive.colon {
        state.take_argument(directive)? == Word::fixnum(1)
    } else {
        state
            .previous_is_one
            .ok_or_else(|| FormatError::at(directive, FormatErrorKind::MissingArgument))?
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
    match directive.kind {
        DirectiveKind::Percent | DirectiveKind::Bar => {
            let character = if directive.kind == DirectiveKind::Percent {
                '\n'
            } else {
                '\u{c}'
            };
            for _ in 0..count {
                output.write_char(character)?;
            }
        }
        DirectiveKind::Ampersand => {
            if count > 0 && !output.line_start {
                output.write_char('\n')?;
            }
            for _ in 1..count {
                output.write_char('\n')?;
            }
        }
        DirectiveKind::Tilde => {
            for _ in 0..count {
                output.write_char('~')?;
            }
        }
        DirectiveKind::Newline => {
            if directive.at_sign {
                output.write_char('\n')?;
            }
        }
        _ => {
            return Err(FormatError::at(
                directive,
                FormatErrorKind::InvalidDirectiveUse,
            ));
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
    match character {
        '\u{0000}' => "Null".to_owned(),
        '\u{0008}' => "Backspace".to_owned(),
        '\t' => "Tab".to_owned(),
        '\n' => "Newline".to_owned(),
        '\u{000c}' => "Page".to_owned(),
        '\r' => "Return".to_owned(),
        ' ' => "Space".to_owned(),
        '\u{007f}' => "Rubout".to_owned(),
        _ => character.to_string(),
    }
}
