use super::{
    Form, FormKind, Runtime, RuntimeError, SymbolTokenKind, atom_name, normalize_name,
    parse_symbol_token, unqualified_name,
};

impl Runtime {
    pub(super) fn list_form_items<'a>(
        form: &'a Form,
        context: &str,
    ) -> Result<&'a [Form], RuntimeError> {
        match &form.kind {
            FormKind::List(items) => Ok(items),
            FormKind::Atom(name) if normalize_name(name) == "NIL" => Ok(&[]),
            _ => Err(Self::invalid(context, form.span)),
        }
    }

    pub(super) fn definition_name_from_form(
        form: &Form,
        context: &str,
    ) -> Result<String, RuntimeError> {
        let Some(raw_name) = atom_name(form) else {
            return Err(Self::invalid(context, form.span));
        };
        let token = parse_symbol_token(raw_name).map_err(|_| Self::invalid(context, form.span))?;
        if !matches!(
            token.kind,
            SymbolTokenKind::Symbol | SymbolTokenKind::Keyword
        ) || token.name.is_empty()
        {
            return Err(Self::invalid(context, form.span));
        }
        if token.escaped && token.package.is_some() {
            return Err(Self::invalid(context, form.span));
        }
        let normalized = if token.escaped {
            token.name
        } else {
            normalize_name(raw_name)
        };
        Ok(unqualified_name(normalized.trim_start_matches(':')))
    }
}
