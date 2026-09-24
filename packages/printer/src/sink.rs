//! Output abstraction for the printer.

use crate::error::PrintError;

/// A character sink the printer writes into.
///
/// `ncl-lib-streams` adapts its streams to this trait so the same printer
/// drives string, file, and terminal output. Tests use [`StringSink`].
pub trait CharSink {
    /// Append one character.
    ///
    /// # Errors
    ///
    /// Returns [`PrintError::Sink`] when the sink cannot accept the character.
    fn write_char(&mut self, character: char) -> Result<(), PrintError>;

    /// Append a string.
    ///
    /// # Errors
    ///
    /// Returns [`PrintError::Sink`] when the sink cannot accept the text.
    fn write_str(&mut self, text: &str) -> Result<(), PrintError> {
        for character in text.chars() {
            self.write_char(character)?;
        }
        Ok(())
    }
}

/// A [`CharSink`] that accumulates its output in a [`String`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StringSink {
    output: String,
}

impl StringSink {
    /// Create an empty sink.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            output: String::new(),
        }
    }

    /// Consume the sink and return the accumulated text.
    #[must_use]
    pub fn into_string(self) -> String {
        self.output
    }

    /// Borrow the accumulated text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.output
    }
}

impl CharSink for StringSink {
    fn write_char(&mut self, character: char) -> Result<(), PrintError> {
        self.output.push(character);
        Ok(())
    }

    fn write_str(&mut self, text: &str) -> Result<(), PrintError> {
        self.output.push_str(text);
        Ok(())
    }
}
