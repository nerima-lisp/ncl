//! Shared-state and bridge adapter support for the pretty printer.

use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use ncl_object::{Runtime, ThreadContext, Word};

use crate::pretty::{IndentMode, NewlineKind, PrettyPrinter, TabKind};
use crate::{CharSink, PrintError, PrintOptions, write};

/// A reference-counted handle for a printer whose state is shared by nested
/// FORMAT and printer operations.
#[derive(Clone, Debug)]
pub struct SharedPrettyPrinter<'a>(Rc<RefCell<PrettyPrinter<'a>>>);

/// Compatibility name for the shared printer lifecycle state.
pub type PrettyPrinterState<'a> = SharedPrettyPrinter<'a>;

impl<'a> SharedPrettyPrinter<'a> {
    /// Put a printer behind a shared, dynamically borrow-checked handle.
    #[must_use]
    pub fn new(printer: PrettyPrinter<'a>) -> Self {
        Self(Rc::new(RefCell::new(printer)))
    }

    /// Borrow the current printer state for inspection.
    #[must_use]
    pub fn borrow(&self) -> Ref<'_, PrettyPrinter<'a>> {
        self.0.borrow()
    }

    /// Borrow the current printer state for one layout operation.
    #[must_use]
    pub fn borrow_mut(&self) -> RefMut<'_, PrettyPrinter<'a>> {
        self.0.borrow_mut()
    }

    /// Create another adapter referring to the same printer state.
    #[must_use]
    pub fn adapter(&self) -> PrettyPrinterAdapter<'a> {
        PrettyPrinterAdapter::new(self.clone())
    }

    /// Flush a trailing conditional break at the end of an output operation.
    ///
    /// # Errors
    ///
    /// Returns a sink error when the pending break cannot be written.
    pub fn finish(&self) -> Result<(), PrintError> {
        self.0.borrow_mut().finish()
    }
}

/// A bridge-facing adapter for a [`SharedPrettyPrinter`].
#[derive(Clone, Debug)]
pub struct PrettyPrinterAdapter<'a> {
    shared: SharedPrettyPrinter<'a>,
}

impl<'a> PrettyPrinterAdapter<'a> {
    /// Create an adapter for shared printer state.
    #[must_use]
    pub const fn new(shared: SharedPrettyPrinter<'a>) -> Self {
        Self { shared }
    }

    /// Return the shared state used by this adapter.
    #[must_use]
    pub fn shared(&self) -> SharedPrettyPrinter<'a> {
        self.shared.clone()
    }

    /// Start a logical block through the shared state.
    ///
    /// # Errors
    ///
    /// Returns a sink error when the prefix cannot be written.
    pub fn start_logical_block(
        &mut self,
        prefix: &str,
        per_line_prefix: Option<&str>,
    ) -> Result<(), PrintError> {
        self.shared
            .borrow_mut()
            .start_logical_block(prefix, per_line_prefix)
    }

    /// End a logical block through the shared state.
    ///
    /// # Errors
    ///
    /// Returns a sink error when the suffix or pending layout cannot be written.
    pub fn end_logical_block(&mut self, suffix: &str) -> Result<(), PrintError> {
        self.shared.borrow_mut().end_logical_block(suffix)
    }

    /// Apply a newline request from a FORMAT bridge.
    ///
    /// # Errors
    ///
    /// Returns a sink error when a mandatory break cannot be written.
    pub fn newline(&mut self, kind: NewlineKind) -> Result<(), PrintError> {
        self.shared.borrow_mut().newline(kind)
    }

    /// Apply an indentation request from a FORMAT bridge.
    pub fn indent(&mut self, mode: IndentMode, amount: isize) {
        self.shared.borrow_mut().indent(mode, amount);
    }

    /// Apply a tabulation request from a FORMAT bridge.
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
        self.shared.borrow_mut().tab(kind, column, increment)
    }

    /// Print an object through the regular printer using this layout state.
    ///
    /// # Errors
    ///
    /// Returns a printer or sink error when the object cannot be written.
    pub fn write_object(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        object: Word,
        options: PrintOptions,
    ) -> Result<(), PrintError> {
        write(ctx, runtime, object, self, &options)
    }

    /// Write the already-rendered segments of a logical block.
    ///
    /// # Errors
    ///
    /// Returns a sink error when layout output cannot be written.
    pub fn logical_block(
        &mut self,
        segments: &[String],
        _colon: bool,
        _at_sign: bool,
    ) -> Result<(), PrintError> {
        let mut printer = self.shared.borrow_mut();
        printer.start_logical_block("", None)?;
        for (index, segment) in segments.iter().enumerate() {
            if index != 0 {
                printer.write_char(' ')?;
            }
            printer.write_str(segment)?;
        }
        printer.end_logical_block("")
    }

    /// Flush pending layout at the end of a bridge operation.
    ///
    /// # Errors
    ///
    /// Returns a sink error when the pending break cannot be written.
    pub fn finish(&mut self) -> Result<(), PrintError> {
        self.shared.finish()
    }
}

impl CharSink for PrettyPrinterAdapter<'_> {
    fn write_char(&mut self, character: char) -> Result<(), PrintError> {
        self.shared.borrow_mut().write_char(character)
    }

    fn write_str(&mut self, text: &str) -> Result<(), PrintError> {
        self.shared.borrow_mut().write_str(text)
    }
}

#[cfg(test)]
mod tests {
    use super::PrettyPrinter;
    use crate::{CharSink, NewlineKind, PrintError, StringSink};

    #[test]
    fn shared_adapter_preserves_state_across_nested_borrows() -> Result<(), PrintError> {
        let mut sink = StringSink::new();
        let shared = PrettyPrinter::with_options(&mut sink, 4, 0).into_shared();
        let mut first = shared.adapter();
        first.write_str("ab")?;
        first.newline(NewlineKind::Linear)?;
        {
            let mut nested = shared.adapter();
            nested.write_str("cd")?;
            nested.finish()?;
        }
        first.finish()?;
        drop(first);
        let column = shared.borrow().column();
        drop(shared);
        assert_eq!(sink.as_str(), "ab cd");
        assert_eq!(column, 5);
        Ok(())
    }

    #[test]
    fn adapter_logical_block_uses_shared_layout_state() -> Result<(), PrintError> {
        let mut sink = StringSink::new();
        let shared = PrettyPrinter::new(&mut sink).into_shared();
        let mut adapter = shared.adapter();
        adapter.logical_block(&["one".to_owned(), "two".to_owned()], true, false)?;
        adapter.finish()?;
        drop(adapter);
        drop(shared);
        assert_eq!(sink.as_str(), "one two");
        Ok(())
    }
}
