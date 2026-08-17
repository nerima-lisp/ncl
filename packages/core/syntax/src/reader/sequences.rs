use super::{MAX_NESTING_DEPTH, Reader};
use crate::{Form, FormKind, ReadError, ReadErrorKind, Span};

impl<'source> Reader<'source> {
    pub(super) fn parse_sequence(&mut self, vector: bool, start: usize) -> Result<Form, ReadError> {
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

    pub(super) fn parse_sequence_inner(
        &mut self,
        vector: bool,
        start: usize,
    ) -> Result<Form, ReadError> {
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
}
