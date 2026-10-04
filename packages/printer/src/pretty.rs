//! Line-oriented pretty-printing primitives shared by printer clients.

use crate::{CharSink, PrintError};

/// The four Common Lisp pretty-printer newline policies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NewlineKind {
    /// Break when the following text would exceed the right margin.
    Linear,
    /// Break when the following text would exceed the right margin, while
    /// allowing a break between successive items in a block.
    Fill,
    /// Break when the current line is within the miser region.
    Miser,
    /// Always break.
    Mandatory,
}

/// The origin used by [`PrettyPrinter::indent`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndentMode {
    /// Count from the start of the logical block.
    Block,
    /// Count from the current column.
    Current,
}

/// The origin used by [`PrettyPrinter::tab`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabKind {
    /// Move to the next tab stop relative to the current column.
    Relative,
    /// Move to an absolute tab stop, then use `increment` for later stops.
    Absolute,
}

#[derive(Clone, Debug)]
struct Block {
    column: usize,
    indent: usize,
    per_line_prefix: Option<String>,
}

/// A small line-oriented pretty printer for a [`CharSink`].
///
/// Text is emitted immediately. A non-mandatory newline is kept pending until
/// the next text arrives, which lets the printer make the margin decision
/// without requiring callers to measure their output first.
pub struct PrettyPrinter<'a> {
    sink: &'a mut dyn CharSink,
    right_margin: usize,
    miser_width: usize,
    column: usize,
    pending: Option<NewlineKind>,
    blocks: Vec<Block>,
    line_count: usize,
}

impl std::fmt::Debug for PrettyPrinter<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrettyPrinter")
            .field("right_margin", &self.right_margin)
            .field("miser_width", &self.miser_width)
            .field("column", &self.column)
            .field("pending", &self.pending)
            .field("blocks", &self.blocks)
            .field("line_count", &self.line_count)
            .finish_non_exhaustive()
    }
}

impl<'a> PrettyPrinter<'a> {
    /// Create a printer with an 80-column right margin.
    pub fn new(sink: &'a mut dyn CharSink) -> Self {
        Self::with_options(sink, 80, 0)
    }

    /// Create a printer with an explicit margin and miser width.
    pub fn with_options(
        sink: &'a mut dyn CharSink,
        right_margin: usize,
        miser_width: usize,
    ) -> Self {
        Self {
            sink,
            right_margin: right_margin.max(1),
            miser_width,
            column: 0,
            pending: None,
            blocks: Vec::new(),
            line_count: 1,
        }
    }

    /// Return the current zero-based column.
    #[must_use]
    pub const fn column(&self) -> usize {
        self.column
    }

    /// Return the configured right margin.
    #[must_use]
    pub const fn right_margin(&self) -> usize {
        self.right_margin
    }

    /// Return the number of lines emitted so far.
    #[must_use]
    pub const fn line_count(&self) -> usize {
        self.line_count
    }

    /// Start a logical block, writing its prefix immediately.
    ///
    /// # Errors
    ///
    /// Returns a sink error when the prefix cannot be written.
    pub fn start_logical_block(
        &mut self,
        prefix: &str,
        per_line_prefix: Option<&str>,
    ) -> Result<(), PrintError> {
        self.write_str(prefix)?;
        self.blocks.push(Block {
            column: self.column,
            indent: 0,
            per_line_prefix: per_line_prefix.map(str::to_owned),
        });
        if let Some(prefix) = per_line_prefix {
            self.write_str(prefix)?;
        }
        Ok(())
    }

    /// End the innermost logical block and write its suffix.
    ///
    /// # Errors
    ///
    /// Returns a sink error when pending layout or the suffix cannot be written.
    pub fn end_logical_block(&mut self, suffix: &str) -> Result<(), PrintError> {
        self.flush_pending(false)?;
        self.blocks.pop();
        self.write_str(suffix)
    }

    /// Request a newline according to `kind`.
    ///
    /// # Errors
    ///
    /// Returns a sink error when a mandatory break must be emitted immediately.
    pub fn newline(&mut self, kind: NewlineKind) -> Result<(), PrintError> {
        if kind == NewlineKind::Mandatory {
            self.pending = None;
            return self.newline_now();
        }
        self.pending = Some(kind);
        Ok(())
    }

    /// Set indentation for the current logical block.
    pub fn indent(&mut self, mode: IndentMode, amount: isize) {
        if let Some(block) = self.blocks.last_mut() {
            let origin = match mode {
                IndentMode::Block => block.column,
                IndentMode::Current => self.column,
            };
            block.indent = if amount.is_negative() {
                origin.saturating_sub(amount.unsigned_abs())
            } else {
                origin.saturating_add(usize::try_from(amount).unwrap_or(usize::MAX))
            };
        }
    }

    /// Move to a tab stop.
    ///
    /// # Errors
    ///
    /// Returns a sink error when padding cannot be written.
    pub fn tab(
        &mut self,
        kind: TabKind,
        column: usize,
        increment: usize,
    ) -> Result<(), PrintError> {
        self.flush_pending(false)?;
        let target = match kind {
            TabKind::Relative => self.column.saturating_add(column),
            TabKind::Absolute => {
                if self.column < column {
                    column
                } else if increment == 0 {
                    self.column
                } else {
                    column
                        + (self
                            .column
                            .saturating_sub(column)
                            .checked_div(increment)
                            .unwrap_or(0)
                            + 1)
                            * increment
                }
            }
        };
        self.write_spaces(target.saturating_sub(self.column))
    }

    /// Write text, resolving any pending conditional newline.
    ///
    /// # Errors
    ///
    /// Returns a sink error from the text or any layout emitted before it.
    pub fn write_str(&mut self, text: &str) -> Result<(), PrintError> {
        if text.is_empty() {
            return Ok(());
        }
        self.flush_pending(self.column + text.chars().count() > self.right_margin)?;
        for character in text.chars() {
            self.write_char(character)?;
        }
        Ok(())
    }

    /// Write one character, resolving any pending conditional newline.
    ///
    /// # Errors
    ///
    /// Returns a sink error when the character or pending layout cannot be written.
    pub fn write_char(&mut self, character: char) -> Result<(), PrintError> {
        self.flush_pending(false)?;
        if character == '\n' {
            self.newline_now()
        } else {
            self.sink.write_char(character)?;
            self.column += 1;
            Ok(())
        }
    }

    fn write_spaces(&mut self, count: usize) -> Result<(), PrintError> {
        for _ in 0..count {
            self.sink.write_char(' ')?;
            self.column += 1;
        }
        Ok(())
    }

    fn flush_pending(&mut self, overflow: bool) -> Result<(), PrintError> {
        let Some(kind) = self.pending.take() else {
            return Ok(());
        };
        let break_line = overflow
            || matches!(kind, NewlineKind::Miser)
                && self.column + self.miser_width >= self.right_margin;
        if break_line {
            self.newline_now()
        } else {
            self.sink.write_char(' ')?;
            self.column += 1;
            Ok(())
        }
    }

    fn newline_now(&mut self) -> Result<(), PrintError> {
        self.sink.write_char('\n')?;
        self.column = 0;
        self.line_count += 1;
        if let Some(block) = self.blocks.last().cloned() {
            self.write_spaces(block.indent)?;
            if let Some(prefix) = block.per_line_prefix {
                self.write_str(&prefix)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{IndentMode, NewlineKind, PrettyPrinter, TabKind};
    use crate::{PrintError, StringSink};

    #[test]
    fn conditional_breaks_use_the_margin_and_block_indent() -> Result<(), PrintError> {
        let mut sink = StringSink::new();
        let mut printer = PrettyPrinter::with_options(&mut sink, 6, 0);
        printer.start_logical_block("(", None)?;
        printer.indent(IndentMode::Block, 2);
        printer.write_str("one")?;
        printer.newline(NewlineKind::Linear)?;
        printer.write_str("two")?;
        printer.end_logical_block(")")?;
        drop(printer);
        assert_eq!(sink.as_str(), "(one\n   two)");
        Ok(())
    }

    #[test]
    fn mandatory_newlines_and_tabs_are_immediate() -> Result<(), PrintError> {
        let mut sink = StringSink::new();
        let mut printer = PrettyPrinter::new(&mut sink);
        printer.write_str("x")?;
        printer.newline(NewlineKind::Mandatory)?;
        printer.tab(TabKind::Relative, 3, 0)?;
        printer.write_str("y")?;
        drop(printer);
        assert_eq!(sink.as_str(), "x\n   y");
        Ok(())
    }

    #[test]
    fn per_line_prefix_is_repeated() -> Result<(), PrintError> {
        let mut sink = StringSink::new();
        let mut printer = PrettyPrinter::with_options(&mut sink, 4, 0);
        printer.start_logical_block("", Some("| "))?;
        printer.write_str("ab")?;
        printer.newline(NewlineKind::Mandatory)?;
        printer.write_str("cd")?;
        drop(printer);
        assert_eq!(sink.as_str(), "| ab\n| cd");
        Ok(())
    }
}
