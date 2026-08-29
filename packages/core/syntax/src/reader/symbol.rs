//! Bare symbol/atom token scanning.

use crate::{Form, ReadError, ReadErrorKind, Reader, Span};

impl Reader<'_> {
    pub(super) fn parse_atom(&mut self) -> Result<Form, ReadError> {
        let start = self.position;
        self.scan_symbol_token(start)?;
        Ok(Form::atom(
            &self.source[start..self.position],
            Span::new(start, self.position),
        ))
    }

    pub(super) fn scan_symbol_token(&mut self, start: usize) -> Result<(), ReadError> {
        let mut in_vertical_bars = false;

        while let Some(character) = self.peek_char() {
            if !in_vertical_bars && Self::is_delimiter(character) {
                break;
            }

            self.position += character.len_utf8();
            match character {
                '|' => in_vertical_bars = !in_vertical_bars,
                '\\' => {
                    let Some(escaped) = self.peek_char() else {
                        return Err(Self::error(
                            ReadErrorKind::UnexpectedEnd { context: "symbol" },
                            Span::new(start, self.position),
                        ));
                    };
                    self.position += escaped.len_utf8();
                }
                _ => {}
            }
        }

        if in_vertical_bars {
            return Err(Self::error(
                ReadErrorKind::UnexpectedEnd { context: "symbol" },
                Span::new(start, self.position),
            ));
        }

        Ok(())
    }
}
