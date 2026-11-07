use ncl_object::{Bignum, ObjectRef, Package, bignum_sign, classify_object, symbol_value};

use crate::executor::{ExecutionState, FormatOutput, integer_value};
use crate::model::Directive;
use crate::{FormatError, FormatErrorKind};

pub(super) fn execute_underscore(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    if !directive.parameters.is_empty() {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        ));
    }
    let should_break = if directive.colon && directive.at_sign {
        true
    } else if output.line_start {
        false
    } else if directive.colon {
        output.column >= output.margin
    } else if directive.at_sign {
        miser_width(state).is_some_and(|width| {
            output.column > output.margin
                && output.block_start >= output.margin.saturating_sub(width)
        })
    } else {
        output.column >= output.margin
    };
    if output.pretty && should_break {
        output.write_char('\n')?;
    }
    Ok(())
}

fn miser_width(state: &mut ExecutionState<'_>) -> Option<usize> {
    let package = state.runtime.find_package(state.ctx, "COMMON-LISP")?;
    let (symbol, _) = Package::from_word(package)
        .intern(state.ctx, state.runtime, "*PRINT-MISER-WIDTH*")
        .ok()?;
    let value = symbol_value(state.ctx, symbol).ok()?;
    match classify_object(state.ctx, value) {
        ObjectRef::Fixnum(width) => usize::try_from(width).ok(),
        ObjectRef::Bignum(word) => {
            let bignum = Bignum::from_word(word);
            if bignum_sign(state.ctx, bignum).ok()? {
                return None;
            }
            Some(
                integer_value(state.ctx, value)
                    .and_then(|width| usize::try_from(width).ok())
                    .unwrap_or(usize::MAX),
            )
        }
        _ => None, // check-added-lines: allow(wildcard) ObjectRef is non-exhaustive.
    }
}
