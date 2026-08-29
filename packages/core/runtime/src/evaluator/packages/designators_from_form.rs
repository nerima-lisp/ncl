use ncl_syntax::{Form, FormKind};

use crate::environment::normalize_name;
use crate::{Runtime, RuntimeError, package};

impl Runtime {
    pub(super) fn package_name_from_form(form: &Form) -> Result<String, RuntimeError> {
        let raw = match &form.kind {
            FormKind::Atom(value) | FormKind::String(value) => value.as_str(),
            _ => {
                return Err(Self::invalid(
                    "package name must be a symbol or string",
                    form.span,
                ));
            }
        };
        if !raw.starts_with(':') && package::split_symbol(raw).is_some() {
            return Err(Self::package_error(
                "package name cannot be qualified",
                form.span,
            ));
        }
        let name = package::normalize_package_name(raw);
        if name.is_empty() || name.contains(':') {
            return Err(Self::package_error("invalid package name", form.span));
        }
        Ok(name)
    }

    pub(super) fn symbol_name_from_form(form: &Form) -> Result<String, RuntimeError> {
        let raw = match &form.kind {
            FormKind::Atom(value) | FormKind::String(value) => value.as_str(),
            _ => {
                return Err(Self::invalid(
                    "symbol name must be a symbol or string",
                    form.span,
                ));
            }
        };
        let name = raw.strip_prefix(':').unwrap_or(raw);
        if name.is_empty() || name.contains(':') {
            return Err(Self::package_error(
                "symbol name cannot be qualified",
                form.span,
            ));
        }
        Ok(normalize_name(name))
    }
}

#[cfg(test)]
mod tests {
    use ncl_syntax::{Form, FormKind, Span};

    use crate::Runtime;

    const SPAN: Span = Span::new(0, 1);

    fn atom(name: &str) -> Form {
        Form::atom(name, SPAN)
    }

    fn string(value: &str) -> Form {
        Form::new(FormKind::String(value.to_string()), SPAN)
    }

    fn valid<T, E>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|_| panic!("expected a valid package or symbol name"))
    }

    #[test]
    fn form_name_helpers_accept_strings_and_keywords() {
        let package_cases = [("foo", "FOO"), (":bar", "BAR"), ("Baz", "BAZ")];
        for (input, expected) in package_cases {
            assert_eq!(
                valid(Runtime::package_name_from_form(&atom(input))),
                expected
            );
        }
        assert_eq!(
            valid(Runtime::package_name_from_form(&string("tools"))),
            "TOOLS"
        );

        let symbol_cases = [("foo", "FOO"), (":bar", "BAR"), ("Baz", "BAZ")];
        for (input, expected) in symbol_cases {
            assert_eq!(
                valid(Runtime::symbol_name_from_form(&atom(input))),
                expected
            );
        }
        assert_eq!(
            valid(Runtime::symbol_name_from_form(&string("tools"))),
            "TOOLS"
        );
    }

    #[test]
    fn form_name_helpers_reject_invalid_designators() {
        let invalid_forms = [
            Form::list(vec![atom("nested")], SPAN),
            atom("foo:bar"),
            atom(":"),
        ];
        for form in invalid_forms {
            assert!(Runtime::package_name_from_form(&form).is_err());
            assert!(Runtime::symbol_name_from_form(&form).is_err());
        }
    }
}
