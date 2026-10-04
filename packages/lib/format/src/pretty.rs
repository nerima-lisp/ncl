use ncl_object::{Runtime, ThreadContext, Word};
use ncl_printer::{CharSink, PrintError, PrintOptions};

/// Pretty-printer newline policy, matching `ncl_printer::NewlineKind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrettyNewline {
    Linear,
    Fill,
    Miser,
    Mandatory,
}

/// Pretty-printer indentation origin, matching `ncl_printer::IndentMode`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrettyIndent {
    Block,
    Current,
}

/// Pretty-printer tab origin, matching `ncl_printer::TabKind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrettyTab {
    Relative,
    Absolute,
}

/// Connection point for the printer-side pretty-printer state machine.
///
/// The object owns the output sink and is itself a `CharSink`, as required by
/// the printer contract. FORMAT only supplies layout requests and object
/// writes; the adapter decides how those requests affect its state.
pub trait PrettyPrinter: CharSink {
    /// Apply a FORMAT newline request.
    ///
    /// # Errors
    /// Returns a sink error when layout output cannot be written.
    fn newline(&mut self, kind: PrettyNewline) -> Result<(), PrintError>;

    /// Apply a FORMAT indentation request.
    fn indent(&mut self, mode: PrettyIndent, amount: isize);

    /// Apply a FORMAT tabulation request.
    ///
    /// # Errors
    /// Returns a sink error when padding cannot be written.
    fn tab(&mut self, kind: PrettyTab, column: usize, increment: usize) -> Result<(), PrintError>;

    /// Print an object through the printer's dispatch and layout state.
    ///
    /// # Errors
    /// Returns a printer error when the object cannot be written.
    fn write_object(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        object: Word,
        options: PrintOptions,
    ) -> Result<(), PrintError>;

    /// Render a `~<...~:>` logical block.
    ///
    /// # Errors
    /// Returns a sink error when block output cannot be written.
    fn logical_block(
        &mut self,
        segments: &[String],
        colon: bool,
        at_sign: bool,
    ) -> Result<(), PrintError>;

    /// Flush a pending conditional break before FORMAT returns.
    ///
    /// # Errors
    /// Returns a sink error when pending layout cannot be written.
    fn finish(&mut self) -> Result<(), PrintError>;
}

/// Adapter used until the printer-side implementation is supplied by the
/// embedding runtime. It deliberately preserves FORMAT's fallback behavior.
#[derive(Debug, Default)]
pub struct NoopPrettyPrinter;

impl CharSink for NoopPrettyPrinter {
    fn write_char(&mut self, _character: char) -> Result<(), PrintError> {
        Ok(())
    }
}

impl PrettyPrinter for NoopPrettyPrinter {
    fn newline(&mut self, _kind: PrettyNewline) -> Result<(), PrintError> {
        Ok(())
    }

    fn indent(&mut self, _mode: PrettyIndent, _amount: isize) {}

    fn tab(
        &mut self,
        _kind: PrettyTab,
        _column: usize,
        _increment: usize,
    ) -> Result<(), PrintError> {
        Ok(())
    }

    fn write_object(
        &mut self,
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _object: Word,
        _options: PrintOptions,
    ) -> Result<(), PrintError> {
        Ok(())
    }

    fn logical_block(
        &mut self,
        _segments: &[String],
        _colon: bool,
        _at_sign: bool,
    ) -> Result<(), PrintError> {
        Ok(())
    }

    fn finish(&mut self) -> Result<(), PrintError> {
        Ok(())
    }
}
