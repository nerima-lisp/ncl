use ncl_object::{ObjectRef, Word, classify_object};

use crate::executor::{Flow, FormatError, FormatErrorKind, execute_control, integer_value};
use crate::model::{ControlSegment, Directive, DirectiveBody, Parameter, TerminatorKind};

use super::{FlowResult, Output, State, invalid_use, missing_argument};

pub(super) fn execute(
    directive: &Directive,
    state: &mut State<'_>,
    output: &mut Output<'_>,
) -> FlowResult {
    let parameters = state.parameters(directive)?;
    if !matches!(
        directive.terminator,
        Some(term)
            if term.kind == TerminatorKind::RightBracket && !term.colon && !term.at_sign
    ) {
        return Err(invalid_use(directive));
    }
    let DirectiveBody::Clauses(clauses) = &directive.body else {
        return Err(invalid_use(directive));
    };
    let selected = if directive.at_sign {
        if directive.colon || !parameters.is_empty() || clauses.len() != 1 {
            return Err(invalid_use(directive));
        }
        validate_separators(clauses, directive, false)?;
        let argument = state
            .arguments
            .get(state.index)
            .copied()
            .ok_or_else(|| missing_argument(directive))?;
        if argument == Word::NIL {
            state.take_argument(directive)?;
            None
        } else {
            Some(0)
        }
    } else if directive.colon {
        if !parameters.is_empty() || clauses.len() != 2 {
            return Err(invalid_use(directive));
        }
        validate_separators(clauses, directive, false)?;
        let argument = state.take_argument(directive)?;
        Some(if argument == Word::NIL { 0 } else { 1 })
    } else {
        validate_separators(clauses, directive, true)?;
        match parameters.first() {
            None | Some(Parameter::Unsupplied) => {
                let selector = state.take_argument(directive)?;
                match classify_object(state.ctx, selector) {
                    ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => {
                        integer_value(state.ctx, selector).and_then(selector_index)
                    }
                    _ => {
                        return Err(FormatError::at(
                            directive,
                            FormatErrorKind::WrongArgumentType,
                        ));
                    }
                }
            }
            Some(Parameter::Integer(value)) => selector_index(*value),
            Some(_) => {
                return Err(FormatError::at(
                    directive,
                    FormatErrorKind::WrongArgumentType,
                ));
            }
        }
    };
    execute_selected(clauses, selected, state, output)
}

fn execute_selected(
    clauses: &[ControlSegment],
    selected: Option<usize>,
    state: &mut State<'_>,
    output: &mut Output<'_>,
) -> FlowResult {
    let default = clauses
        .last()
        .and_then(|clause| clause.separator.as_ref())
        .is_some_and(|separator| separator.colon);
    let index = selected
        .filter(|index| *index < clauses.len())
        .or_else(|| default.then_some(clauses.len().saturating_sub(1)));
    match index {
        Some(index) => execute_control(state, &clauses[index].control, output),
        None => Ok(Flow::Continue),
    }
}

fn validate_separators(
    clauses: &[ControlSegment],
    directive: &Directive,
    allow_default: bool,
) -> Result<(), FormatError> {
    for (index, clause) in clauses.iter().enumerate() {
        let Some(separator) = &clause.separator else {
            continue;
        };
        if separator.at_sign || !separator.parameters.is_empty() {
            return Err(invalid_use(directive));
        }
        if separator.colon && (!allow_default || index + 1 != clauses.len()) {
            return Err(invalid_use(directive));
        }
    }
    Ok(())
}

fn selector_index(value: i64) -> Option<usize> {
    usize::try_from(value).ok()
}
