use crate::numeric::is_valid_radix_integer_literal;
use crate::{parse_symbol_token, Form, FormKind, ReadError, ReadErrorKind, Span, SymbolTokenKind};

// Keep the recursive first reader below the platform stack limit.
pub const MAX_NESTING_DEPTH: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Feature {
    kind: SymbolTokenKind,
    package: Option<String>,
    name: String,
}

enum ParsedForm {
    Form(Form),
    Skipped,
    End,
}

pub struct Reader<'source> {
    source: &'source str,
    position: usize,
    nesting_depth: usize,
    form_depth: usize,
    quasiquote_depth: usize,
    features: Vec<Feature>,
}

impl<'source> Reader<'source> {
    #[must_use]
    pub fn new(source: &'source str) -> Self {
        Self {
            source,
            position: 0,
            nesting_depth: 0,
            form_depth: 0,
            quasiquote_depth: 0,
            features: Vec::new(),
        }
    }

    pub fn with_features<I, S>(source: &'source str, features: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut reader = Self::new(source);
        reader.features = features
            .into_iter()
            .filter_map(|feature| Self::parse_feature_token(feature.as_ref()))
            .collect();
        reader
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
        if let Some(character) = self.peek_char()
            && character.is_whitespace()
        {
            self.position += character.len_utf8();
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
        loop {
            match self.parse_form_once()? {
                ParsedForm::Form(form) => return Ok(Some(form)),
                ParsedForm::Skipped => continue,
                ParsedForm::End => return Ok(None),
            }
        }
    }

    fn parse_form_once(&mut self) -> Result<ParsedForm, ReadError> {
        if self.form_depth >= MAX_NESTING_DEPTH {
            return Err(self.error(
                ReadErrorKind::NestingTooDeep {
                    limit: MAX_NESTING_DEPTH,
                },
                Span::new(self.position, self.position),
            ));
        }
        self.form_depth += 1;
        let result = self.parse_form_inner();
        self.form_depth -= 1;
        result
    }

    fn parse_form_inner(&mut self) -> Result<ParsedForm, ReadError> {
        self.skip_ignored()?;
        let Some(character) = self.peek_char() else {
            return Ok(ParsedForm::End);
        };
        match character {
            '(' => self
                .parse_sequence(false, self.position)
                .map(ParsedForm::Form),
            ')' => Err(self.error(
                ReadErrorKind::UnexpectedClosingDelimiter { delimiter: ')' },
                Span::new(self.position, self.position + 1),
            )),
            '\'' => self.parse_prefix("quote", 1).map(ParsedForm::Form),
            '\x60' => self.parse_prefix("quasiquote", 1).map(ParsedForm::Form),
            ',' => {
                let start = self.position;
                self.position += 1;
                if self.peek_char() == Some('@') {
                    self.position += 1;
                    self.parse_prefixed_form("unquote-splicing", start, self.position)
                        .map(ParsedForm::Form)
                } else {
                    self.parse_prefixed_form("unquote", start, self.position)
                        .map(ParsedForm::Form)
                }
            }
            '"' => self.parse_string().map(ParsedForm::Form),
            '#' => self.parse_dispatch(),
            _ => self.parse_atom().map(ParsedForm::Form),
        }
    }

    fn parse_prefix(
        &mut self,
        name: &'static str,
        prefix_length: usize,
    ) -> Result<Form, ReadError> {
        let start = self.position;
        self.position += prefix_length;
        self.parse_prefixed_form(name, start, self.position)
    }

    fn parse_prefixed_form(
        &mut self,
        name: &'static str,
        start: usize,
        prefix_end: usize,
    ) -> Result<Form, ReadError> {
        let previous_quasiquote_depth = self.quasiquote_depth;
        match name {
            "quasiquote" => self.quasiquote_depth += 1,
            "unquote" | "unquote-splicing" => {
                if self.quasiquote_depth == 0 {
                    return Err(self.error(
                        ReadErrorKind::UnquoteOutsideQuasiquote,
                        Span::new(start, prefix_end),
                    ));
                }
                self.quasiquote_depth -= 1;
            }
            _ => {}
        }

        let form_result = self.parse_form();
        self.quasiquote_depth = previous_quasiquote_depth;
        let Some(form) = form_result? else {
            return Err(self.error(
                ReadErrorKind::UnexpectedEnd { context: name },
                Span::new(start, self.position),
            ));
        };
        let end = form.span.end;
        Ok(Form::list(
            vec![Form::atom(name, Span::new(start, prefix_end)), form],
            Span::new(start, end),
        ))
    }

    fn parse_dispatch(&mut self) -> Result<ParsedForm, ReadError> {
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
                Ok(ParsedForm::Skipped)
            }
            '+' | '-' => self.parse_reader_conditional(start, character == '+'),
            '(' => self.parse_sequence(true, start).map(ParsedForm::Form),
            '\'' => {
                self.position += 1;
                self.parse_prefixed_form("function", start, self.position)
                    .map(ParsedForm::Form)
            }
            '\\' => self.parse_character(start).map(ParsedForm::Form),
            ':' => self.parse_uninterned_symbol(start).map(ParsedForm::Form),
            '*' => self.parse_bit_vector(start).map(ParsedForm::Form),
            'c' | 'C' => self.parse_complex(start).map(ParsedForm::Form),
            '.' => {
                self.position += 1;
                let Some(form) = self.parse_form()? else {
                    return Err(self.error(
                        ReadErrorKind::UnexpectedEnd {
                            context: "read-time evaluation",
                        },
                        Span::new(start, self.position),
                    ));
                };
                let end = form.span.end;
                Ok(ParsedForm::Form(Form::new(
                    FormKind::ReadTimeEval(Box::new(form)),
                    Span::new(start, end),
                )))
            }
            'b' | 'B' => self.parse_radix_integer(start, 2).map(ParsedForm::Form),
            'o' | 'O' => self.parse_radix_integer(start, 8).map(ParsedForm::Form),
            'x' | 'X' => self.parse_radix_integer(start, 16).map(ParsedForm::Form),
            '0'..='9' => self
                .parse_general_radix_integer(start)
                .map(ParsedForm::Form),
            't' | 'T' => {
                self.position += 1;
                self.ensure_dispatch_boundary(start)?;
                Ok(ParsedForm::Form(Form::atom(
                    "#t",
                    Span::new(start, self.position),
                )))
            }
            'f' | 'F' => {
                self.position += 1;
                self.ensure_dispatch_boundary(start)?;
                Ok(ParsedForm::Form(Form::atom(
                    "#f",
                    Span::new(start, self.position),
                )))
            }
            _ => Err(self.error(ReadErrorKind::InvalidDispatch, Span::new(start, start + 1))),
        }
    }

    fn parse_reader_conditional(
        &mut self,
        start: usize,
        positive: bool,
    ) -> Result<ParsedForm, ReadError> {
        self.position += 1;
        let Some(feature_form) = self.parse_form()? else {
            return Err(self.error(
                ReadErrorKind::UnexpectedEnd {
                    context: "reader conditional feature expression",
                },
                Span::new(start, self.position),
            ));
        };
        let enabled = self.evaluate_feature_expression(&feature_form)?;
        let branch = self.parse_form_once()?;
        match branch {
            ParsedForm::Form(form) if enabled == positive => Ok(ParsedForm::Form(form)),
            ParsedForm::Form(_) | ParsedForm::Skipped => Ok(ParsedForm::Skipped),
            ParsedForm::End => Err(self.error(
                ReadErrorKind::UnexpectedEnd {
                    context: "reader conditional form",
                },
                Span::new(start, self.position),
            )),
        }
    }

    fn evaluate_feature_expression(&self, form: &Form) -> Result<bool, ReadError> {
        match &form.kind {
            FormKind::Atom(token) => {
                let feature = self.feature_from_token(token, form.span)?;
                Ok(self.features.iter().any(|candidate| candidate == &feature))
            }
            FormKind::List(items) => {
                let Some((operator, operands)) = items.split_first() else {
                    return Err(self.error(ReadErrorKind::InvalidDispatch, form.span));
                };
                let FormKind::Atom(token) = &operator.kind else {
                    return Err(self.error(ReadErrorKind::InvalidDispatch, operator.span));
                };
                let operator_span = operator.span;
                let operator = parse_symbol_token(token)
                    .map_err(|_| self.error(ReadErrorKind::InvalidDispatch, operator_span))?;
                if operator.kind == SymbolTokenKind::Uninterned || operator.package.is_some() {
                    return Err(self.error(ReadErrorKind::InvalidDispatch, operator_span));
                }
                match operator.name.as_str() {
                    "AND" => {
                        let mut enabled = true;
                        for operand in operands {
                            if !self.evaluate_feature_expression(operand)? {
                                enabled = false;
                            }
                        }
                        Ok(enabled)
                    }
                    "OR" => {
                        let mut enabled = false;
                        for operand in operands {
                            if self.evaluate_feature_expression(operand)? {
                                enabled = true;
                            }
                        }
                        Ok(enabled)
                    }
                    "NOT" if operands.len() == 1 => {
                        Ok(!self.evaluate_feature_expression(&operands[0])?)
                    }
                    _ => Err(self.error(ReadErrorKind::InvalidDispatch, form.span)),
                }
            }
            _ => Err(self.error(ReadErrorKind::InvalidDispatch, form.span)),
        }
    }

    fn feature_from_token(&self, token: &str, span: Span) -> Result<Feature, ReadError> {
        let token = parse_symbol_token(token)
            .map_err(|_| self.error(ReadErrorKind::InvalidDispatch, span))?;
        if token.kind == SymbolTokenKind::Uninterned {
            return Err(self.error(ReadErrorKind::InvalidDispatch, span));
        }
        Ok(Feature {
            kind: token.kind,
            package: token.package,
            name: token.name,
        })
    }

    fn parse_feature_token(token: &str) -> Option<Feature> {
        let token = parse_symbol_token(token).ok()?;
        if token.kind == SymbolTokenKind::Uninterned {
            return None;
        }
        Some(Feature {
            kind: token.kind,
            package: token.package,
            name: token.name,
        })
    }

    fn parse_complex(&mut self, start: usize) -> Result<Form, ReadError> {
        self.position += 1;
        if self.peek_char() != Some('(') {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            ));
        }
        let form = self.parse_sequence(false, start)?;
        let end = form.span.end;
        match form.kind {
            FormKind::List(mut items) if items.len() == 2 => {
                let imaginary = items.pop().expect("complex form has two elements");
                let real = items.pop().expect("complex form has two elements");
                Ok(Form::new(
                    FormKind::Complex {
                        real: Box::new(real),
                        imaginary: Box::new(imaginary),
                    },
                    Span::new(start, end),
                ))
            }
            _ => Err(self.error(ReadErrorKind::InvalidDispatch, Span::new(start, end))),
        }
    }

    fn parse_bit_vector(&mut self, start: usize) -> Result<Form, ReadError> {
        self.position += 1;
        let mut bits = Vec::new();
        while let Some(character) = self.peek_char() {
            match character {
                '0' => bits.push(0),
                '1' => bits.push(1),
                _ if self.is_delimiter(character) => break,
                _ => {
                    return Err(self.error(
                        ReadErrorKind::InvalidDispatch,
                        Span::new(start, self.position + character.len_utf8()),
                    ));
                }
            }
            self.position += character.len_utf8();
        }
        Ok(Form::new(
            FormKind::BitVector(bits),
            Span::new(start, self.position),
        ))
    }

    fn parse_radix_integer(&mut self, start: usize, base: u32) -> Result<Form, ReadError> {
        self.position += 1;
        if matches!(self.peek_char(), Some('+' | '-')) {
            self.position += 1;
        }
        let digits_start = self.position;
        let mut invalid = false;
        while let Some(character) = self.peek_char() {
            if self.is_delimiter(character) {
                break;
            }
            if !character.is_ascii_alphanumeric() || character.to_digit(base).is_none() {
                invalid = true;
            }
            self.position += character.len_utf8();
        }
        if self.position == digits_start || invalid {
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

    fn parse_general_radix_integer(&mut self, start: usize) -> Result<Form, ReadError> {
        while self
            .peek_char()
            .is_some_and(|character| character.is_ascii_digit())
        {
            self.position += 1;
        }
        if !matches!(self.peek_char(), Some('r' | 'R')) {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            ));
        }
        self.position += 1;
        if matches!(self.peek_char(), Some('+' | '-')) {
            self.position += 1;
        }
        let digits_start = self.position;
        let mut invalid = false;
        while let Some(character) = self.peek_char() {
            if self.is_delimiter(character) {
                break;
            }
            if !character.is_ascii_alphanumeric() {
                invalid = true;
            }
            self.position += character.len_utf8();
        }
        let literal = &self.source[start..self.position];
        if self.position == digits_start || invalid || !is_valid_radix_integer_literal(literal) {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            ));
        }
        Ok(Form::atom(literal, Span::new(start, self.position)))
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
                if items.is_empty() {
                    return Err(self.error(
                        ReadErrorKind::MissingDottedHead,
                        Span::new(start, self.position + 1),
                    ));
                }
                self.position += 1;
                self.skip_ignored()?;
                if self.peek_char() == Some('.') && self.dot_is_standalone() {
                    return Err(self.error(
                        ReadErrorKind::MultipleDottedTails,
                        Span::new(self.position, self.position + 1),
                    ));
                }
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
                    if self.peek_char() == Some('.') && self.dot_is_standalone() {
                        return Err(self.error(
                            ReadErrorKind::MultipleDottedTails,
                            Span::new(self.position, self.position + 1),
                        ));
                    }
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
            match self.parse_form_once()? {
                ParsedForm::Form(form) => items.push(form),
                ParsedForm::Skipped => continue,
                ParsedForm::End => {
                    return Err(self.error(
                        ReadErrorKind::UnexpectedEnd {
                            context: "list item",
                        },
                        Span::new(start, self.position),
                    ));
                }
            }
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
                    value.push(escaped);
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
            "backspace" => '\u{0008}',
            "linefeed" => '\n',
            "page" => '\u{000c}',
            "space" => ' ',
            "newline" => '\n',
            "tab" => '\t',
            "return" => '\r',
            "rubout" => '\u{007f}',
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

pub fn read_with_features<I, S>(source: &str, features: I) -> Result<Vec<Form>, ReadError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Reader::with_features(source, features).read_all()
}
