//! Dispatch-macro (`#...`) parsing: booleans, characters, complex literals, uninterned symbols, radix integers.

use crate::{
    Form, FormKind, ReadError, ReadErrorKind, Reader, Span, SymbolTokenKind, parse_symbol_token,
};

impl Reader<'_> {
    pub(super) fn parse_dispatch(&mut self) -> Result<Option<Form>, ReadError> {
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
            'c' | 'C' => self.parse_complex_literal(start),
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

    fn parse_complex_literal(&mut self, start: usize) -> Result<Option<Form>, ReadError> {
        self.position += 1;
        if self.peek_char() != Some('(') {
            return Err(Self::error(
                ReadErrorKind::InvalidDispatch,
                Span::new(start, self.position),
            ));
        }
        let form = self.parse_sequence(false, start)?;
        let Form {
            kind: FormKind::List(mut items),
            span,
        } = form
        else {
            unreachable!("complex literal parser requests a list")
        };
        if items.len() != 2 {
            return Err(Self::error(ReadErrorKind::InvalidDispatch, span));
        }
        items.insert(0, Form::atom("complex", Span::new(start, start + 2)));
        Ok(Some(Form::list(items, span)))
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
}
