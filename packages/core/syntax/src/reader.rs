use crate::{Form, ReadError, ReadErrorKind, Span};
use std::collections::HashSet;

mod dispatch;
mod sequence;
mod string_char;
mod symbol;
mod whitespace;

/// Maximum supported recursive reader nesting.
pub const MAX_NESTING_DEPTH: usize = 256;

/// Stateful reader for NCL source text.
#[derive(Debug)]
pub struct Reader<'source> {
    source: &'source str,
    position: usize,
    nesting_depth: usize,
    features: HashSet<String>,
}

impl<'source> Reader<'source> {
    /// Creates a reader positioned at the beginning of `source`.
    #[must_use]
    pub fn new(source: &'source str) -> Self {
        Self::with_features(source, [":ncl"])
    }

    /// Creates a reader with the supplied reader conditional features.
    pub fn with_features<I, S>(source: &'source str, features: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            source,
            position: 0,
            nesting_depth: 0,
            features: features
                .into_iter()
                .map(|feature| Self::normalize_feature(feature.as_ref()))
                .collect(),
        }
    }

    fn normalize_feature(feature: &str) -> String {
        feature.trim_start_matches(':').to_ascii_lowercase()
    }

    /// Returns the current byte position.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    /// Reads the next form, or `None` at end of input.
    ///
    /// # Errors
    ///
    /// Returns a [`ReadError`] when the source is malformed.
    pub fn read_form(&mut self) -> Result<Option<Form>, ReadError> {
        self.skip_ignored()?;
        if self.position >= self.source.len() {
            return Ok(None);
        }
        self.parse_form()
    }

    /// Consumes one whitespace character after a form, when present.
    pub fn consume_one_whitespace_after_form(&mut self) {
        if let Some(character) = self.peek_char()
            && character.is_whitespace()
        {
            self.position += character.len_utf8();
        }
    }

    /// Reads every form in the remaining source.
    ///
    /// # Errors
    ///
    /// Returns a [`ReadError`] when any form is malformed.
    pub fn read_all(&mut self) -> Result<Vec<Form>, ReadError> {
        let mut forms = Vec::new();
        while let Some(form) = self.read_form()? {
            forms.push(form);
        }
        Ok(forms)
    }

    fn parse_form(&mut self) -> Result<Option<Form>, ReadError> {
        self.skip_ignored()?;
        let Some(character) = self.peek_char() else {
            return Ok(None);
        };
        match character {
            '(' => self.parse_sequence(false, self.position).map(Some),
            ')' => Err(Self::error(
                ReadErrorKind::UnexpectedClosingDelimiter { delimiter: ')' },
                Span::new(self.position, self.position + 1),
            )),
            '\'' => self.parse_prefix("quote", 1),
            '\x60' => self.parse_prefix("quasiquote", 1),
            ',' => {
                let start = self.position;
                self.position += 1;
                if self.peek_char() == Some('@') {
                    self.position += 1;
                    self.parse_prefixed_form("unquote-splicing", start, self.position)
                } else {
                    self.parse_prefixed_form("unquote", start, self.position)
                }
            }
            '"' => self.parse_string().map(Some),
            '#' => self.parse_dispatch(),
            _ => self.parse_atom().map(Some),
        }
    }

    fn parse_prefix(
        &mut self,
        name: &'static str,
        prefix_length: usize,
    ) -> Result<Option<Form>, ReadError> {
        let start = self.position;
        self.position += prefix_length;
        self.parse_prefixed_form(name, start, self.position)
    }

    fn parse_prefixed_form(
        &mut self,
        name: &'static str,
        start: usize,
        prefix_end: usize,
    ) -> Result<Option<Form>, ReadError> {
        let Some(form) = self.parse_form()? else {
            return Err(Self::error(
                ReadErrorKind::UnexpectedEnd { context: name },
                Span::new(start, self.position),
            ));
        };
        let end = form.span.end;
        Ok(Some(Form::list(
            vec![Form::atom(name, Span::new(start, prefix_end)), form],
            Span::new(start, end),
        )))
    }

    const fn is_delimiter(character: char) -> bool {
        character.is_whitespace()
            || matches!(character, '(' | ')' | '"' | '\'' | '\x60' | ',' | ';' | '#')
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }

    const fn error(kind: ReadErrorKind, span: Span) -> ReadError {
        ReadError::new(kind, span)
    }
}

/// Reads all forms from `source`.
///
/// # Errors
///
/// Returns a [`ReadError`] when the source is malformed.
pub fn read(source: &str) -> Result<Vec<Form>, ReadError> {
    Reader::new(source).read_all()
}
