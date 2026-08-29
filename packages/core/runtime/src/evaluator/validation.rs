#[allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn arity(function: &str, expected: &str, actual: usize) -> RuntimeError {
        RuntimeError::Arity {
            function: function.to_string(),
            expected: expected.to_string(),
            actual,
        }
    }

    pub(super) fn block_name(form: &Form) -> Result<String, RuntimeError> {
        let FormKind::Atom(name) = &form.kind else {
            return Err(Self::invalid("block name must be a symbol", form.span));
        };
        if name.is_empty() || (name.starts_with(':') && name.len() == 1) {
            return Err(Self::invalid("block name must be a symbol", form.span));
        }
        if !name.starts_with(':')
            && literal_atom(name).is_some()
            && !name.eq_ignore_ascii_case("nil")
            && !name.eq_ignore_ascii_case("t")
        {
            return Err(Self::invalid("block name must be a symbol", form.span));
        }
        Ok(normalize_name(name))
    }

    pub(super) fn restart_name(form: &Form) -> Result<String, RuntimeError> {
        let FormKind::Atom(name) = &form.kind else {
            return Err(Self::invalid("restart name must be a symbol", form.span));
        };
        if name.is_empty() || (name.starts_with(':') && name.len() == 1) {
            return Err(Self::invalid("restart name must be a symbol", form.span));
        }
        if !name.starts_with(':')
            && literal_atom(name).is_some()
            && !name.eq_ignore_ascii_case("nil")
            && !name.eq_ignore_ascii_case("t")
        {
            return Err(Self::invalid("restart name must be a symbol", form.span));
        }
        Ok(normalize_name(name))
    }

    pub(super) fn condition_name(form: &Form) -> Result<String, RuntimeError> {
        let Some(name) = atom_name(form) else {
            return Err(Self::invalid("condition name must be a symbol", form.span));
        };
        if name.is_empty()
            || (name.starts_with(':') && name.len() == 1)
            || (!name.starts_with(':')
                && literal_atom(name).is_some()
                && !name.eq_ignore_ascii_case("nil")
                && !name.eq_ignore_ascii_case("t"))
        {
            return Err(Self::invalid("condition name must be a symbol", form.span));
        }
        Ok(normalize_name(name).trim_start_matches(':').to_string())
    }

    pub(super) fn variable_name_info(
        form: &Form,
        context: &str,
    ) -> Result<(String, bool), RuntimeError> {
        let Some(name) = atom_name(form) else {
            return Err(Self::invalid(context, form.span));
        };
        let Ok(token) = parse_symbol_token(name) else {
            return Err(Self::invalid(context, form.span));
        };
        if token.kind != SymbolTokenKind::Symbol
            || token.name.is_empty()
            || (token.escaped && token.package.is_some())
            || (!token.escaped && (token.name.starts_with('&') || literal_atom(name).is_some()))
        {
            return Err(Self::invalid(context, form.span));
        }
        let variable_name = if token.escaped {
            token.name
        } else {
            normalize_name(name)
        };
        Ok((variable_name, token.escaped))
    }

    pub(super) fn variable_name(form: &Form, context: &str) -> Result<String, RuntimeError> {
        Self::variable_name_info(form, context).map(|(name, _)| name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(name: &str) -> Form {
        Form::atom(name, Span::new(2, 2 + name.len()))
    }

    #[test]
    fn validates_name_categories_from_table_cases() {
        let valid = ["name", ":keyword", "nil", "t", "|literal|", "foo\\ bar"];
        for name in valid {
            assert!(Runtime::block_name(&atom(name)).is_ok(), "block: {name}");
            assert!(
                Runtime::restart_name(&atom(name)).is_ok(),
                "restart: {name}"
            );
        }

        let invalid = ["", ":", "1", "(list)"];
        for name in invalid {
            let form = if name == "(list)" {
                Form::list(vec![atom("list")], Span::new(0, 6))
            } else {
                atom(name)
            };
            assert!(Runtime::block_name(&form).is_err(), "block: {name}");
            assert!(Runtime::restart_name(&form).is_err(), "restart: {name}");
        }

        for name in ["condition", ":condition", "nil", "t"] {
            assert!(
                Runtime::condition_name(&atom(name)).is_ok(),
                "condition: {name}"
            );
        }
        for name in ["", ":", "1"] {
            assert!(
                Runtime::condition_name(&atom(name)).is_err(),
                "condition: {name}"
            );
        }
    }

    #[test]
    fn validates_variable_names_and_preserves_escaping() {
        let cases = [
            ("name", "NAME", false),
            ("|Name|", "Name", true),
            ("foo\\ bar", "FOO BAR", true),
        ];
        for (source, expected, escaped) in cases {
            assert_eq!(
                Runtime::variable_name_info(&atom(source), "variable"),
                Ok((expected.into(), escaped))
            );
        }
        for source in ["", "&optional", "1", ":keyword"] {
            assert!(
                Runtime::variable_name(&atom(source), "variable").is_err(),
                "variable: {source}"
            );
        }
        let non_atom = Form::list(Vec::new(), Span::new(0, 2));
        assert!(Runtime::variable_name(&non_atom, "variable").is_err());
    }
}
