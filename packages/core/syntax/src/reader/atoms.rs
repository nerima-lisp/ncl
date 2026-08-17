use super::Reader;
use crate::{Form, FormKind, ReadError, ReadErrorKind, Span};

impl<'source> Reader<'source> {
    pub(super) fn parse_string(&mut self) -> Result<Form, ReadError> {
        let start = self.position;
        self.position += 1;
        let mut value = String::new();
        while let Some(character) = self.peek_char() {
            self.position += character.len_utf8();
            match character {
                '"' => {
                    return Ok(Form::new(
                        FormKind::String(value),
                        Span::new(start, self.position),
                    ));
                }
                '\\' => {
                    let Some(escaped) = self.peek_char() else {
                        return Err(self.error(
                            ReadErrorKind::UnexpectedEnd { context: "string" },
                            Span::new(start, self.position),
                        ));
                    };
                    self.position += escaped.len_utf8();
                    let decoded = match escaped {
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        '\\' => '\\',
                        '"' => '"',
                        _ => {
                            return Err(self.error(
                                ReadErrorKind::InvalidEscape,
                                Span::new(self.position - escaped.len_utf8() - 1, self.position),
                            ));
                        }
                    };
                    value.push(decoded);
                }
                _ => value.push(character),
            }
        }
        Err(self.error(
            ReadErrorKind::UnexpectedEnd { context: "string" },
            Span::new(start, self.position),
        ))
    }

    pub(super) fn parse_character(&mut self, start: usize) -> Result<Form, ReadError> {
        self.position += 1;
        let token_start = self.position;
        if let Some(character) = self.peek_char()
            && self.is_delimiter(character)
        {
            self.position += character.len_utf8();
            return Ok(Form::new(
                FormKind::Character(character),
                Span::new(start, self.position),
            ));
        }
        while let Some(character) = self.peek_char() {
            if self.is_delimiter(character) {
                break;
            }
            self.position += character.len_utf8();
        }
        let token = &self.source[token_start..self.position];
        let value = match token.to_ascii_lowercase().as_str() {
            "space" => ' ',
            "newline" => '\n',
            "tab" => '\t',
            "return" => '\r',
            "" => {
                return Err(self.error(
                    ReadErrorKind::InvalidCharacterName,
                    Span::new(start, self.position),
                ));
            }
            _ => {
                let mut characters = token.chars();
                let Some(value) = characters.next() else {
                    return Err(self.error(
                        ReadErrorKind::InvalidCharacterName,
                        Span::new(start, self.position),
                    ));
                };
                if characters.next().is_some() {
                    return Err(self.error(
                        ReadErrorKind::InvalidCharacterName,
                        Span::new(start, self.position),
                    ));
                }
                value
            }
        };
        Ok(Form::new(
            FormKind::Character(value),
            Span::new(start, self.position),
        ))
    }

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
            if !in_vertical_bars && self.is_delimiter(character) {
                break;
            }

            self.position += character.len_utf8();
            match character {
                '|' => in_vertical_bars = !in_vertical_bars,
                '\\' => {
                    let Some(escaped) = self.peek_char() else {
                        return Err(self.error(
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
            return Err(self.error(
                ReadErrorKind::UnexpectedEnd { context: "symbol" },
                Span::new(start, self.position),
            ));
        }

        Ok(())
    }
}
