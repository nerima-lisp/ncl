use super::Reader;
use crate::{Form, ReadError, ReadErrorKind, Span};

impl<'source> Reader<'source> {
    pub(super) fn parse_form(&mut self) -> Result<Option<Form>, ReadError> {
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

    pub(super) fn parse_prefix(
        &mut self,
        name: &'static str,
        prefix_length: usize,
    ) -> Result<Option<Form>, ReadError> {
        let start = self.position;
        self.position += prefix_length;
        self.parse_prefixed_form(name, start, self.position)
    }

    pub(super) fn parse_prefixed_form(
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
}
