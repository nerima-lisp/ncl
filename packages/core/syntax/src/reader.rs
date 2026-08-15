use crate::{
    parse_symbol_token, Form, FormKind, ReadError, ReadErrorKind, Span, SymbolTokenKind,
};

// Keep the recursive first reader below the platform stack limit.
pub const MAX_NESTING_DEPTH: usize = 256;

pub struct Reader<'source> {
    source: &'source str,
    position: usize,
    nesting_depth: usize,
}

impl<'source> Reader<'source> {
    pub fn new(source: &'source str) -> Self {
        Self {
            source,
            position: 0,
            nesting_depth: 0,
        }
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn read_form(&mut self) -> Result<Option<Form>, ReadError> {
        self.skip_ignored()?;
        if self.position >= self.source.len() {
            return Ok(None);
        }
        self.parse_form()
    }

    pub fn consume_one_whitespace_after_form(&mut self) {
        if let Some(character) = self.peek_char() {
            if character.is_whitespace() {
                self.position += character.len_utf8();
            }
        }
    }

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
            ')' => Err(self.error(
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
            return Err(self.error(
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
            return Err(self.error(
                ReadErrorKind::UnexpectedEnd {
                    context: "dispatch macro",
                },
                Span::new(start, self.position),
            ));
        };
        if character.is_ascii_digit() {
            return self.parse_numeric_dispatch(start).map(Some);
        }
        match character {
            ';' => {
                self.position += 1;
                let Some(_) = self.parse_form()? else {
                    return Err(self.error(
                        ReadErrorKind::UnexpectedEnd {
                            context: "discarded form",
                        },
                        Span::new(start, self.position),
                    ));
                };
                self.parse_form()
            }
            '(' => self.parse_sequence(true, start).map(Some),
            '*' => self.parse_bit_vector(start).map(Some),
            'b' | 'B' => self.parse_radix_integer(start, 2).map(Some),
            'o' | 'O' => self.parse_radix_integer(start, 8).map(Some),
            'x' | 'X' => self.parse_radix_integer(start, 16).map(Some),
            'c' | 'C' => self.parse_complex_literal(start).map(Some),
            's' | 'S' => self.parse_structure_literal(start).map(Some),
            'p' | 'P' => self.parse_pathname_literal(start).map(Some),
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
            _ => Err(self.error(ReadErrorKind::InvalidDispatch, Span::new(start, start + 1))),
        }
    }

    fn parse_numeric_dispatch(&mut self, start: usize) -> Result<Form, ReadError> {
        let digits_start = self.position;
        while matches!(self.peek_char(), Some(character) if character.is_ascii_digit()) {
            self.position += 1;
        }

        let rank = self.source[digits_start..self.position]
            .parse::<usize>()
            .map_err(|_| self.error(ReadErrorKind::InvalidDispatch, Span::new(start, self.position)))?;
        match self.peek_char() {
            Some('a') | Some('A') => self.parse_array_literal(start, rank),
            _ => Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            )),
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
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            ));
        }
        Ok(Form::atom(
            &self.source[start..self.position],
            Span::new(start, self.position),
        ))
    }

    fn parse_bit_vector(&mut self, start: usize) -> Result<Form, ReadError> {
        self.position += 1;
        let mut items = Vec::new();

        while let Some(character) = self.peek_char() {
            if self.is_delimiter(character) {
                break;
            }

            let item_start = self.position;
            self.position += character.len_utf8();
            match character {
                '0' | '1' => items.push(Form::atom(
                    &self.source[item_start..self.position],
                    Span::new(item_start, self.position),
                )),
                _ => {
                    return Err(
                        self.error(ReadErrorKind::InvalidDispatch, Span::new(start, self.position))
                    );
                }
            }
        }

        Ok(Form::new(FormKind::Vector(items), Span::new(start, self.position)))
    }

    fn parse_radix_integer(&mut self, start: usize, radix: u32) -> Result<Form, ReadError> {
        self.position += 1;
        let digits_start = self.position;

        if matches!(self.peek_char(), Some('+') | Some('-')) {
            self.position += 1;
        }

        let mut saw_digit = false;
        while let Some(character) = self.peek_char() {
            if self.is_delimiter(character) {
                break;
            }

            self.position += character.len_utf8();
            if character.to_digit(radix).is_some() {
                saw_digit = true;
                continue;
            }

            return Err(self.error(ReadErrorKind::InvalidDispatch, Span::new(start, self.position)));
        }

        if !saw_digit || self.position == digits_start {
            return Err(self.error(ReadErrorKind::InvalidDispatch, Span::new(start, self.position)));
        }

        let token = &self.source[digits_start..self.position];
        let value = i64::from_str_radix(
            token.strip_prefix('+').or_else(|| token.strip_prefix('-')).unwrap_or(token),
            radix,
        )
        .map_err(|_| self.error(ReadErrorKind::InvalidDispatch, Span::new(start, self.position)))?;
        let normalized = if token.starts_with('-') {
            format!("-{value}")
        } else {
            value.to_string()
        };

        Ok(Form::atom(&normalized, Span::new(start, self.position)))
    }

    fn parse_structure_literal(&mut self, start: usize) -> Result<Form, ReadError> {
        self.position += 1;
        self.skip_ignored()?;
        let Some(form) = self.parse_form()? else {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            ));
        };

        let FormKind::List(items) = &form.kind else {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, form.span.end),
            ));
        };
        if items.is_empty() || items.len() % 2 == 0 {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, form.span.end),
            ));
        }

        let FormKind::Atom(name) = &items[0].kind else {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, items[0].span.end),
            ));
        };

        let constructor = format!("MAKE-{name}");
        let mut rewritten = Vec::with_capacity(items.len());
        rewritten.push(Form::atom(
            &constructor,
            Span::new(start, items[0].span.end),
        ));
        rewritten.extend(items.iter().skip(1).cloned());
        Ok(Form::list(rewritten, Span::new(start, form.span.end)))
    }

    fn parse_complex_literal(&mut self, start: usize) -> Result<Form, ReadError> {
        self.position += 1;
        self.skip_ignored()?;
        let Some(form) = self.parse_form()? else {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            ));
        };

        let FormKind::List(items) = &form.kind else {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, form.span.end),
            ));
        };
        let [real, imag] = items.as_slice() else {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, form.span.end),
            ));
        };

        Ok(Form::list(
            vec![
                Form::atom("complex", Span::new(start, start + 1)),
                real.clone(),
                imag.clone(),
            ],
            Span::new(start, form.span.end),
        ))
    }

    fn parse_pathname_literal(&mut self, start: usize) -> Result<Form, ReadError> {
        self.position += 1;
        self.skip_ignored()?;
        let Some(form) = self.parse_form()? else {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            ));
        };

        match form.kind {
            FormKind::String(value) => Ok(Form::new(
                FormKind::String(value),
                Span::new(start, form.span.end),
            )),
            _ => Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, form.span.end),
            )),
        }
    }

    fn parse_array_literal(&mut self, start: usize, rank: usize) -> Result<Form, ReadError> {
        if rank == 0 {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            ));
        }

        self.position += 1;
        self.skip_ignored()?;
        let Some(contents) = self.parse_form()? else {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            ));
        };

        let dimensions = self.array_literal_dimensions(&contents, rank, start)?;
        let end = contents.span.end;
        let span = Span::new(start, end);
        let dimensions_form = Form::list(
            dimensions
                .iter()
                .map(|dimension| Form::atom(dimension.to_string(), contents.span))
                .collect(),
            contents.span,
        );

        Ok(Form::list(
            vec![
                Form::atom("make-array", Span::new(start, start + 1)),
                self.quote_form(dimensions_form, span),
                Form::atom(":initial-contents", contents.span),
                self.quote_form(contents, span),
            ],
            span,
        ))
    }

    fn array_literal_dimensions(
        &self,
        form: &Form,
        rank: usize,
        start: usize,
    ) -> Result<Vec<usize>, ReadError> {
        let FormKind::List(items) = &form.kind else {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, form.span.end),
            ));
        };

        if rank == 1 {
            return Ok(vec![items.len()]);
        }

        let Some((first, rest)) = items.split_first() else {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, form.span.end),
            ));
        };

        let nested = self.array_literal_dimensions(first, rank - 1, start)?;
        for item in rest {
            let candidate = self.array_literal_dimensions(item, rank - 1, start)?;
            if candidate != nested {
                return Err(self.error(
                    ReadErrorKind::InvalidDispatch,
                    Span::new(start, item.span.end),
                ));
            }
        }

        let mut dimensions = Vec::with_capacity(rank);
        dimensions.push(items.len());
        dimensions.extend(nested);
        Ok(dimensions)
    }

    fn quote_form(&self, form: Form, span: Span) -> Form {
        Form::list(vec![Form::atom("quote", span), form], span)
    }

    fn parse_sequence(&mut self, vector: bool, start: usize) -> Result<Form, ReadError> {
        if self.nesting_depth >= MAX_NESTING_DEPTH {
            return Err(self.error(
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
                return Err(self.error(
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
                    return Err(self.error(
                        ReadErrorKind::MissingDottedTail,
                        Span::new(self.position, self.position + 1),
                    ));
                }
                let Some(tail) = self.parse_form()? else {
                    return Err(self.error(
                        ReadErrorKind::MissingDottedTail,
                        Span::new(start, self.position),
                    ));
                };
                self.skip_ignored()?;
                if self.peek_char() != Some(closing) {
                    let Some(found) = self.peek_char() else {
                        return Err(self.error(
                            ReadErrorKind::UnexpectedEnd {
                                context: "dotted list",
                            },
                            Span::new(start, self.position),
                        ));
                    };
                    return Err(self.error(
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
                return Err(self.error(
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

    fn parse_character(&mut self, start: usize) -> Result<Form, ReadError> {
        self.position += 1;
        let token_start = self.position;
        if let Some(character) = self.peek_char() {
            if self.is_delimiter(character) {
                self.position += character.len_utf8();
                return Ok(Form::new(
                    FormKind::Character(character),
                    Span::new(start, self.position),
                ));
            }
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

        Err(self.error(
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
            .is_none_or(|character| self.is_delimiter(character))
    }

    fn is_delimiter(&self, character: char) -> bool {
        character.is_whitespace()
            || matches!(character, '(' | ')' | '"' | '\'' | '\x60' | ',' | ';' | '#')
    }

    fn ensure_dispatch_boundary(&self, start: usize) -> Result<(), ReadError> {
        if let Some(character) = self.peek_char() {
            if !self.is_delimiter(character) {
                return Err(self.error(
                    ReadErrorKind::InvalidDispatch,
                    Span::new(start, self.position + character.len_utf8()),
                ));
            }
        }
        Ok(())
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }

    fn error(&self, kind: ReadErrorKind, span: Span) -> ReadError {
        ReadError::new(kind, span)
    }
}

pub fn read(source: &str) -> Result<Vec<Form>, ReadError> {
    Reader::new(source).read_all()
}
