use ncl_object::{Runtime, ThreadContext, Word};
use ncl_printer::{CharSink, PrintError, PrintOptions};

use crate::Directive;

/// A pretty-print operation emitted by FORMAT's layout directives.
#[derive(Debug)]
pub enum PrettyOperation<'a> {
    /// Request a `pprint-newline` operation for `~_`.
    Newline(&'a Directive),
    /// Request a `pprint-indent` operation for `~I`.
    Indent(&'a Directive),
    /// Request a printer-controlled `~W` write.
    Write {
        /// The object being printed.
        object: Word,
        /// The print options selected by the directive.
        options: PrintOptions,
    },
    /// Request a logical block for `~<~:>`.
    LogicalBlock {
        /// Rendered logical-block segments.
        segments: &'a [String], // check-added-lines: allow(index) This is a slice type, not indexing.
        /// Whether the directive has its colon modifier.
        colon: bool,
        /// Whether the directive has its at-sign modifier.
        at_sign: bool,
    },
}

/// Connection point for the runtime's pretty printer.
pub trait PrettyPrinter {
    /// Handle an operation and return whether it replaced FORMAT's fallback.
    ///
    /// # Errors
    /// Returns [`PrintError`] when the pretty printer cannot write to `stream`.
    fn handle(
        &mut self,
        operation: PrettyOperation<'_>,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        stream: &mut dyn CharSink,
    ) -> Result<bool, PrintError>;
}

/// Temporary fallback until the printer-side pprint state machine lands.
#[derive(Debug, Default)]
pub struct NoopPrettyPrinter;

impl PrettyPrinter for NoopPrettyPrinter {
    fn handle(
        &mut self,
        _operation: PrettyOperation<'_>,
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _stream: &mut dyn CharSink,
    ) -> Result<bool, PrintError> {
        Ok(false)
    }
}
