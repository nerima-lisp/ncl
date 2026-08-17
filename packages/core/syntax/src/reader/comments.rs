use super::Reader;
use crate::{ReadError, ReadErrorKind, Span};

impl<'source> Reader<'source> {
    pub(super) fn skip_ignored(&mut self) -> Result<(), ReadError> {
        loop {
            while let Some(character) = self.peek_char() {
                if character.is_whitespace() {
                    self.position += character.len_utf8();
                } else {
                    break;
                }
            }
            if self.peek_char() == Some(';') {
                while let Some(character) = self.peek_char() {
                    self.position += character.len_utf8();
                    if character == '\n' {
                        break;
                    }
                }
                continue;
            }
            if self.source[self.position..].starts_with("#|") {
                self.skip_block_comment()?;
                continue;
            }
            return Ok(());
        }
    }

    pub(super) fn skip_block_comment(&mut self) -> Result<(), ReadError> {
        let start = self.position;
        let mut depth = 0;

        while self.position < self.source.len() {
            if self.source[self.position..].starts_with("#|") {
                self.position += 2;
                depth += 1;
            } else if self.source[self.position..].starts_with("|#") {
                self.position += 2;
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            } else if let Some(character) = self.peek_char() {
                self.position += character.len_utf8();
            }
        }

        Err(self.error(
            ReadErrorKind::UnexpectedEnd {
                context: "block comment",
            },
            Span::new(start, self.position),
        ))
    }

    pub(super) fn dot_is_standalone(&self) -> bool {
        self.source[self.position + 1..]
            .chars()
            .next()
            .is_none_or(|character| self.is_delimiter(character))
    }

    pub(super) fn is_delimiter(&self, character: char) -> bool {
        character.is_whitespace()
            || matches!(character, '(' | ')' | '"' | '\'' | '\x60' | ',' | ';' | '#')
    }

    pub(super) fn ensure_dispatch_boundary(&self, start: usize) -> Result<(), ReadError> {
        if let Some(character) = self.peek_char()
            && !self.is_delimiter(character)
        {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position + character.len_utf8()),
            ));
        }
        Ok(())
    }

    pub(super) fn peek_char(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }

    pub(super) fn error(&self, kind: ReadErrorKind, span: Span) -> ReadError {
        ReadError::new(kind, span)
    }
}
