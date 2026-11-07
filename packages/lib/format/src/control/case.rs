use ncl_printer::StringSink;

use crate::executor::execute_control;
use crate::model::{Directive, DirectiveBody, TerminatorKind};

use super::{FlowResult, Output, State, invalid_use};

pub(super) fn execute(
    directive: &Directive,
    state: &mut State<'_>,
    output: &mut Output<'_>,
) -> FlowResult {
    let parameters = state.parameters(directive)?;
    let Some(terminator) = directive.terminator else {
        return Err(invalid_use(directive));
    };
    let DirectiveBody::Sequence(body) = &directive.body else {
        return Err(invalid_use(directive));
    };
    if !parameters.is_empty()
        || terminator.kind != TerminatorKind::RightParen
        || terminator.colon
        || terminator.at_sign
    {
        return Err(invalid_use(directive));
    }
    let mut sink = StringSink::new();
    let flow = {
        let mut nested_output = crate::executor::FormatOutput::new(&mut sink);
        nested_output.column = output.column;
        nested_output.line_start = output.line_start;
        execute_control(state, body, &mut nested_output)?
    };
    output.write_str(&case_text(
        &sink.into_string(),
        directive.colon,
        directive.at_sign,
    ))?;
    Ok(flow)
}

fn case_text(text: &str, colon: bool, at_sign: bool) -> String {
    match (colon, at_sign) {
        (false, false) => text.chars().flat_map(char::to_lowercase).collect(),
        (true, false) => capitalize_words(text),
        (false, true) => capitalize_first_word(text),
        (true, true) => text.chars().flat_map(char::to_uppercase).collect(),
    }
}

fn capitalize_words(text: &str) -> String {
    let mut in_word = false;
    let mut result = String::new();
    for character in text.chars() {
        if character.is_alphanumeric() {
            if in_word {
                result.extend(character.to_lowercase());
            } else {
                result.extend(character.to_uppercase());
            }
            in_word = true;
        } else {
            result.push(character);
            in_word = false;
        }
    }
    result
}

fn capitalize_first_word(text: &str) -> String {
    let mut found_word = false;
    let mut result = String::new();
    for character in text.chars() {
        if character.is_alphanumeric() {
            if found_word {
                result.extend(character.to_lowercase());
            } else {
                result.extend(character.to_uppercase());
                found_word = true;
            }
        } else {
            result.push(character);
        }
    }
    result
}
