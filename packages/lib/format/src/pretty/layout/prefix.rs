use ncl_printer::StringSink;

use crate::FormatError;
use crate::executor::{ExecutionState, Flow, FormatOutput, execute_control};
use crate::model::FormatControl;

pub(super) fn execute_directive(
    state: &mut ExecutionState<'_>,
    control: &FormatControl,
    output: &mut FormatOutput<'_>,
    prefix: &str,
    changes_indentation: bool,
) -> Result<Flow, FormatError> {
    if output.line_start {
        write_line_prefix(output, prefix)?;
    }
    let original_indentation = output.indentation;
    let mut sink = StringSink::new();
    let (result, indentation, block_start, margin, pretty) = {
        let mut buffered = FormatOutput::new(&mut sink);
        buffered.column = output.column;
        buffered.line_start = output.line_start;
        buffered.margin = output.margin;
        buffered.indentation = 0;
        buffered.block_start = output.block_start;
        buffered.pretty = output.pretty;
        let result = execute_control(state, control, &mut buffered);
        let indentation = if changes_indentation && output.pretty {
            buffered.indentation
        } else {
            original_indentation
        };
        (
            result,
            indentation,
            buffered.block_start,
            buffered.margin,
            buffered.pretty,
        )
    };
    let rendered = sink.into_string();
    let write_result = write_text(&rendered, output, prefix);
    output.indentation = indentation;
    output.block_start = block_start;
    output.margin = margin;
    output.pretty = pretty;
    write_result?;
    result
}

pub(super) fn write_text(
    text: &str,
    output: &mut FormatOutput<'_>,
    prefix: &str,
) -> Result<(), FormatError> {
    for character in text.chars() {
        if output.line_start {
            write_line_prefix(output, prefix)?;
        }
        output.write_char(character)?;
    }
    Ok(())
}

fn write_line_prefix(output: &mut FormatOutput<'_>, prefix: &str) -> Result<(), FormatError> {
    let indentation = output.indentation;
    output.indentation = 0;
    let result = (|| {
        output.write_str(prefix)?;
        let padding = indentation.saturating_sub(output.column);
        for _ in 0..padding {
            output.write_char(' ')?;
        }
        Ok(())
    })();
    output.indentation = indentation;
    result
}

pub(super) fn write_fill_literal(
    text: &str,
    output: &mut FormatOutput<'_>,
    prefix: &str,
) -> Result<(), FormatError> {
    let mut remaining = text;
    while let Some(index) = remaining.find(' ') {
        let (word, tail) = remaining.split_at(index);
        write_text(word, output, prefix)?;
        let rest = &tail[1..];
        let next_word = rest
            .chars()
            .take_while(|character| !character.is_whitespace())
            .count();
        let width = super::effective_column(output)
            .saturating_add(1)
            .saturating_add(next_word);
        if !output.line_start && width > output.margin {
            output.write_char('\n')?;
        } else {
            output.write_char(' ')?;
        }
        remaining = rest;
    }
    write_text(remaining, output, prefix)
}
