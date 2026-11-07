use crate::executor::FormatOutput;
use crate::model::Directive;
use crate::{FormatError, FormatErrorKind};

const MISER_WIDTH: usize = 40;

pub(super) fn execute_underscore(
    directive: &Directive,
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
        output.column > output.margin
            && output.block_start >= output.margin.saturating_sub(MISER_WIDTH)
    } else {
        output.column >= output.margin
    };
    if output.pretty && should_break {
        output.write_char('\n')?;
    }
    Ok(())
}
