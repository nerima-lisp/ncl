use super::Reader;
use crate::{Form, FormKind, ReadError, ReadErrorKind, Span, SymbolTokenKind, parse_symbol_token};

impl<'source> Reader<'source> {
    pub(super) fn parse_uninterned_symbol(&mut self, start: usize) -> Result<Form, ReadError> {
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

    pub(super) fn parse_bit_vector(&mut self, start: usize) -> Result<Form, ReadError> {
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
                    return Err(self.error(
                        ReadErrorKind::InvalidDispatch,
                        Span::new(start, self.position),
                    ));
                }
            }
        }

        Ok(Form::new(
            FormKind::Vector(items),
            Span::new(start, self.position),
        ))
    }

    pub(super) fn parse_structure_literal(&mut self, start: usize) -> Result<Form, ReadError> {
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

    pub(super) fn parse_complex_literal(&mut self, start: usize) -> Result<Form, ReadError> {
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

    pub(super) fn parse_pathname_literal(&mut self, start: usize) -> Result<Form, ReadError> {
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

    pub(super) fn parse_array_literal(
        &mut self,
        start: usize,
        rank: usize,
    ) -> Result<Form, ReadError> {
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

    pub(super) fn array_literal_dimensions(
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

    pub(super) fn quote_form(&self, form: Form, span: Span) -> Form {
        Form::list(vec![Form::atom("quote", span), form], span)
    }
}
