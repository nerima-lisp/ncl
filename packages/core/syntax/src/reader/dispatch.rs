use super::Reader;
use crate::{Form, ReadError, ReadErrorKind, Span};

impl<'source> Reader<'source> {
    pub(super) fn parse_dispatch(&mut self) -> Result<Option<Form>, ReadError> {
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
                self.parse_form()
            }
            '+' => self.parse_conditional(start, true),
            '-' => self.parse_conditional(start, false),
            '(' => self.parse_sequence(true, start).map(Some),
            '*' => self.parse_bit_vector(start).map(Some),
            'b' | 'B' => self.parse_radix_integer(start, 2).map(Some),
            'c' | 'C' => self.parse_complex_literal(start).map(Some),
            'o' | 'O' => self.parse_radix_integer(start, 8).map(Some),
            'p' | 'P' => self.parse_pathname_literal(start).map(Some),
            '\'' => {
                self.position += 1;
                self.parse_prefixed_form("function", start, self.position)
            }
            '\\' => self.parse_character(start).map(Some),
            ':' => self.parse_uninterned_symbol(start).map(Some),
            's' | 'S' => self.parse_structure_literal(start).map(Some),
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
            'x' | 'X' => self.parse_radix_integer(start, 16).map(Some),
            character if character.is_ascii_digit() => self.parse_numeric_dispatch(start).map(Some),
            _ => Err(self.error(ReadErrorKind::InvalidDispatch, Span::new(start, start + 1))),
        }
    }

    pub(super) fn parse_numeric_dispatch(&mut self, start: usize) -> Result<Form, ReadError> {
        let digits_start = self.position;
        while matches!(self.peek_char(), Some(character) if character.is_ascii_digit()) {
            self.position += 1;
        }

        let radix_or_rank = &self.source[digits_start..self.position];
        match self.peek_char() {
            Some('a') | Some('A') => {
                let rank = radix_or_rank.parse::<usize>().map_err(|_| {
                    self.error(
                        ReadErrorKind::InvalidDispatch,
                        Span::new(start, self.position),
                    )
                })?;
                self.parse_array_literal(start, rank)
            }
            Some('r') | Some('R') => {
                let radix = radix_or_rank.parse::<u32>().map_err(|_| {
                    self.error(
                        ReadErrorKind::InvalidDispatch,
                        Span::new(start, self.position),
                    )
                })?;
                self.position += 1;
                if !(2..=36).contains(&radix) {
                    return Err(self.error(
                        ReadErrorKind::InvalidDispatch,
                        Span::new(start, self.position),
                    ));
                }
                self.parse_radix_digits(start, radix)
            }
            _ => Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            )),
        }
    }

    pub(super) fn parse_radix_integer(
        &mut self,
        start: usize,
        radix: u32,
    ) -> Result<Form, ReadError> {
        self.position += 1;
        self.parse_radix_digits(start, radix)
    }

    pub(super) fn parse_radix_digits(
        &mut self,
        start: usize,
        radix: u32,
    ) -> Result<Form, ReadError> {
        let token_start = self.position;
        self.scan_symbol_token(token_start)?;
        self.ensure_dispatch_boundary(start)?;

        let token = &self.source[token_start..self.position];
        let (negative, digits) = match token.chars().next() {
            Some('+') => (false, &token['+'.len_utf8()..]),
            Some('-') => (true, &token['-'.len_utf8()..]),
            Some(_) => (false, token),
            None => {
                return Err(self.error(
                    ReadErrorKind::InvalidDispatch,
                    Span::new(start, self.position),
                ));
            }
        };

        if digits.is_empty() {
            return Err(self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            ));
        }

        let mut value = 0_i128;
        for character in digits.chars() {
            let Some(digit) = character.to_digit(radix) else {
                return Err(self.error(
                    ReadErrorKind::InvalidDispatch,
                    Span::new(start, self.position),
                ));
            };
            value = value
                .checked_mul(i128::from(radix))
                .and_then(|value| value.checked_add(i128::from(digit)))
                .ok_or_else(|| {
                    self.error(
                        ReadErrorKind::InvalidDispatch,
                        Span::new(start, self.position),
                    )
                })?;
        }

        let value = if negative { -value } else { value };
        let value = i64::try_from(value).map_err(|_| {
            self.error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            )
        })?;
        Ok(Form::atom(
            value.to_string(),
            Span::new(start, self.position),
        ))
    }
}
