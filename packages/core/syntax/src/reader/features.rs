use super::{Reader, normalize_feature_name};
use crate::{Form, FormKind, ReadError, ReadErrorKind, Span, SymbolTokenKind, parse_symbol_token};

impl<'source> Reader<'source> {
    pub(super) fn parse_conditional(
        &mut self,
        start: usize,
        include_when_present: bool,
    ) -> Result<Option<Form>, ReadError> {
        self.position += 1;
        let feature = self.parse_form()?.ok_or_else(|| {
            self.error(
                ReadErrorKind::UnexpectedEnd {
                    context: "feature expression",
                },
                Span::new(start, self.position),
            )
        })?;
        let enabled = self.feature_expression_enabled(&feature)?;
        let branch = self.parse_form()?.ok_or_else(|| {
            self.error(
                ReadErrorKind::UnexpectedEnd {
                    context: "conditional form",
                },
                Span::new(start, self.position),
            )
        })?;
        if enabled == include_when_present {
            Ok(Some(branch))
        } else {
            self.parse_form()
        }
    }

    pub(super) fn feature_expression_enabled(&self, form: &Form) -> Result<bool, ReadError> {
        match &form.kind {
            FormKind::Atom(name) => self.feature_atom_enabled(name, form.span),
            FormKind::List(items) => {
                let Some(operator_form) = items.first() else {
                    return Err(self.error(ReadErrorKind::InvalidDispatch, form.span));
                };
                let FormKind::Atom(operator) = &operator_form.kind else {
                    return Err(self.error(ReadErrorKind::InvalidDispatch, operator_form.span));
                };
                let token = parse_symbol_token(operator)
                    .map_err(|_| self.error(ReadErrorKind::InvalidDispatch, operator_form.span))?;
                if token.package.is_some() || matches!(&token.kind, SymbolTokenKind::Uninterned) {
                    return Err(self.error(ReadErrorKind::InvalidDispatch, operator_form.span));
                }
                match token.name.to_ascii_uppercase().as_str() {
                    "AND" => {
                        let mut enabled = true;
                        for item in &items[1..] {
                            enabled = enabled && self.feature_expression_enabled(item)?;
                        }
                        Ok(enabled)
                    }
                    "OR" => {
                        let mut enabled = false;
                        for item in &items[1..] {
                            enabled = enabled || self.feature_expression_enabled(item)?;
                        }
                        Ok(enabled)
                    }
                    "NOT" if items.len() == 2 => {
                        let enabled = self.feature_expression_enabled(&items[1])?;
                        Ok(!enabled)
                    }
                    _ => Err(self.error(ReadErrorKind::InvalidDispatch, operator_form.span)),
                }
            }
            _ => Err(self.error(ReadErrorKind::InvalidDispatch, form.span)),
        }
    }

    pub(super) fn feature_atom_enabled(&self, name: &str, span: Span) -> Result<bool, ReadError> {
        let token = parse_symbol_token(name)
            .map_err(|_| self.error(ReadErrorKind::InvalidDispatch, span))?;
        if token.package.is_some() || matches!(&token.kind, SymbolTokenKind::Uninterned) {
            return Err(self.error(ReadErrorKind::InvalidDispatch, span));
        }
        Ok(self.features.contains(&normalize_feature_name(&token.name)))
    }
}
