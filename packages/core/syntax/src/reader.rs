use crate::{Form, FormKind, ReadError, ReadErrorKind, Span, SymbolTokenKind, parse_symbol_token};

/// Maximum supported recursive reader nesting.
pub const MAX_NESTING_DEPTH: usize = 256;

/// Stateful reader for NCL source text.
#[derive(Debug)]
pub struct Reader<'source> {
    source: &'source str,
    position: usize,
    nesting_depth: usize,
}

impl<'source> Reader<'source> {
    /// Creates a reader positioned at the beginning of `source`.
    #[must_use]
    pub const fn new(source: &'source str) -> Self {
        Self {
            source,
            position: 0,
            nesting_depth: 0,
        }
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

    fn parse_dispatch(&mut self) -> Result<Option<Form>, ReadError> {
        let start = self.position;
        self.position += 1;
        let Some(character) = self.peek_char() else {
            return Err(Self::error(
                ReadErrorKind::UnexpectedEnd {
                    context: "dispatch macro",
                },
                Span::new(start, self.position),
            ));
        };
        match character {
            ';' => {
                self.position += 1;
                let Some(_) = self.parse_form()? else {
                    return Err(Self::error(
                        ReadErrorKind::UnexpectedEnd {
                            context: "discarded form",
                        },
                        Span::new(start, self.position),
                    ));
                };
                self.parse_form()
            }
            '(' => self.parse_sequence(true, start).map(Some),
            '\'' => {
                self.position += 1;
                self.parse_prefixed_form("function", start, self.position)
            }
            '\\' => self.parse_character(start).map(Some),
            ':' => self.parse_uninterned_symbol(start).map(Some),
            't' | 'T' => {
                self.position += 1;
                self.ensure_dispatch_boundary(start)?;
                Ok(Some(Form::atom("#t", Span::new(start, self.position))))
            }
            'f' | 'F' => {
                self.position += 1;
                self.ensure_dispatch_boundary(start)?;
                Ok(Some(Form::atom("#f", Span::new(start, self.position))))
            }
            'b' | 'B' | 'o' | 'O' | 'x' | 'X' | '0'..='9' => {
                self.parse_radix_integer(start).map(Some)
            }
            _ => Err(Self::error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, start + 1),
            )),
        }
    }

    fn parse_radix_integer(&mut self, start: usize) -> Result<Form, ReadError> {
        self.scan_symbol_token(start)?;
        let token = &self.source[start..self.position];
        if crate::numeric::is_valid_radix_integer_literal(token) {
            Ok(Form::atom(token, Span::new(start, self.position)))
        } else {
            Err(Self::error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            ))
        }
    }

    fn parse_uninterned_symbol(&mut self, start: usize) -> Result<Form, ReadError> {
        self.position += 1;
        self.scan_symbol_token(start)?;
        let token = parse_symbol_token(&self.source[start..self.position]);
        if !matches!(
            token,
            Ok(ref token)
                if token.kind == SymbolTokenKind::Uninterned && token.package.is_none()
        ) {
            return Err(Self::error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            ));
        }
        Ok(Form::atom(
            &self.source[start..self.position],
            Span::new(start, self.position),
        ))
    }

    fn parse_sequence(&mut self, vector: bool, start: usize) -> Result<Form, ReadError> {
        if self.nesting_depth >= MAX_NESTING_DEPTH {
            return Err(Self::error(
                ReadErrorKind::NestingTooDeep {
                    limit: MAX_NESTING_DEPTH,
                },
                Span::new(start, self.position + 1),
            ));
        }
        self.nesting_depth += 1;
        let result = self.parse_sequence_inner(vector, start);
        self.nesting_depth -= 1;
        result
    }

    fn parse_sequence_inner(&mut self, vector: bool, start: usize) -> Result<Form, ReadError> {
        self.position += 1;
        let closing = ')';
        let mut items = Vec::new();

        loop {
            self.skip_ignored()?;
            let Some(character) = self.peek_char() else {
                return Err(Self::error(
                    ReadErrorKind::UnexpectedEnd { context: "list" },
                    Span::new(start, self.position),
                ));
            };
            if character == closing {
                self.position += 1;
                let span = Span::new(start, self.position);
                return if vector {
                    Ok(Form::new(FormKind::Vector(items), span))
                } else {
                    Ok(Form::list(items, span))
                };
            }
            if !vector && character == '.' && self.dot_is_standalone() {
                self.position += 1;
                self.skip_ignored()?;
                if self.peek_char() == Some(closing) {
                    return Err(Self::error(
                        ReadErrorKind::MissingDottedTail,
                        Span::new(self.position, self.position + 1),
                    ));
                }
                let Some(tail) = self.parse_form()? else {
                    return Err(Self::error(
                        ReadErrorKind::MissingDottedTail,
                        Span::new(start, self.position),
                    ));
                };
                self.skip_ignored()?;
                if self.peek_char() != Some(closing) {
                    let Some(found) = self.peek_char() else {
                        return Err(Self::error(
                            ReadErrorKind::UnexpectedEnd {
                                context: "dotted list",
                            },
                            Span::new(start, self.position),
                        ));
                    };
                    return Err(Self::error(
                        ReadErrorKind::MismatchedDelimiter {
                            expected: closing,
                            found,
                        },
                        Span::new(self.position, self.position + found.len_utf8()),
                    ));
                }
                self.position += 1;
                return Ok(Form::dotted_list(
                    items,
                    tail,
                    Span::new(start, self.position),
                ));
            }
            let Some(form) = self.parse_form()? else {
                return Err(Self::error(
                    ReadErrorKind::UnexpectedEnd {
                        context: "list item",
                    },
                    Span::new(start, self.position),
                ));
            };
            items.push(form);
        }
    }

    fn parse_string(&mut self) -> Result<Form, ReadError> {
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
                        return Err(Self::error(
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
                            return Err(Self::error(
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
        Err(Self::error(
            ReadErrorKind::UnexpectedEnd { context: "string" },
            Span::new(start, self.position),
        ))
    }

    fn parse_character(&mut self, start: usize) -> Result<Form, ReadError> {
        self.position += 1;
        let token_start = self.position;
        if let Some(character) = self.peek_char()
            && Self::is_delimiter(character)
        {
            self.position += character.len_utf8();
            return Ok(Form::new(
                FormKind::Character(character),
                Span::new(start, self.position),
            ));
        }
        while let Some(character) = self.peek_char() {
            if Self::is_delimiter(character) {
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
                return Err(Self::error(
                    ReadErrorKind::InvalidCharacterName,
                    Span::new(start, self.position),
                ));
            }
            _ => {
                let mut characters = token.chars();
                let Some(value) = characters.next() else {
                    unreachable!("non-empty character token")
                };
                if characters.next().is_some() {
                    return Err(Self::error(
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

    fn parse_atom(&mut self) -> Result<Form, ReadError> {
        let start = self.position;
        self.scan_symbol_token(start)?;
        Ok(Form::atom(
            &self.source[start..self.position],
            Span::new(start, self.position),
        ))
    }

    fn scan_symbol_token(&mut self, start: usize) -> Result<(), ReadError> {
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

    fn skip_ignored(&mut self) -> Result<(), ReadError> {
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

    fn dot_is_standalone(&self) -> bool {
        self.source[self.position + 1..]
            .chars()
            .next()
            .is_none_or(Self::is_delimiter)
    }

    const fn is_delimiter(character: char) -> bool {
        character.is_whitespace()
            || matches!(character, '(' | ')' | '"' | '\'' | '\x60' | ',' | ';' | '#')
    }

    fn ensure_dispatch_boundary(&self, start: usize) -> Result<(), ReadError> {
        if let Some(character) = self.peek_char()
            && !Self::is_delimiter(character)
        {
            return Err(Self::error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position + character.len_utf8()),
            ));
        }
        Ok(())
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
