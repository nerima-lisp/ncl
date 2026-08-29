//! Whitespace, line-comment, and block-comment skipping.

use crate::{ReadError, ReadErrorKind, Reader, Span};

impl Reader<'_> {
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

    fn skip_block_comment(&mut self) -> Result<(), ReadError> {
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

        Err(Self::error(
            ReadErrorKind::UnexpectedEnd {
                context: "block comment",
            },
            Span::new(start, self.position),
        ))
    }
}
