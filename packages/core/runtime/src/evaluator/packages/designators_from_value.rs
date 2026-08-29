use ncl_syntax::Span;

use crate::evaluator::helpers::unqualified_name;
use crate::{Runtime, RuntimeError, Value, package};

impl Runtime {
    pub(in crate::evaluator) fn package_designator_name(
        value: &Value,
        span: Span,
    ) -> Result<String, RuntimeError> {
        let raw = match value {
            Value::Package(name) | Value::String(name) => name.as_ref(),
            _ => value.symbol_name().ok_or_else(|| RuntimeError::Type {
                expected: "PACKAGE DESIGNATOR".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            })?,
        };
        if package::split_symbol(raw).is_some() {
            return Err(Self::package_error(
                "package name cannot be qualified",
                span,
            ));
        }
        let name = package::normalize_package_name(raw);
        if name.is_empty() || name.contains(':') {
            return Err(Self::package_error("invalid package name", span));
        }
        Ok(name)
    }

    pub(in crate::evaluator) fn package_name_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<String, RuntimeError> {
        let name = Self::package_designator_name(value, span)?;
        let packages = self.packages.borrow();
        if !packages.package_exists(&name) {
            return Err(Self::package_error(
                &format!("unknown package {name}"),
                span,
            ));
        }
        Ok(packages.canonical_package_name(&name))
    }

    pub(in crate::evaluator) fn symbol_name_from_value(
        value: &Value,
        span: Span,
    ) -> Result<String, RuntimeError> {
        let raw = match value {
            Value::String(name) => name.as_ref(),
            _ => value.symbol_name().ok_or_else(|| RuntimeError::Type {
                expected: "STRING DESIGNATOR".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            })?,
        };
        let name = raw.strip_prefix(':').unwrap_or(raw);
        if name.is_empty() || package::split_symbol(name).is_some() || name.contains(':') {
            return Err(Self::package_error("symbol name cannot be qualified", span));
        }
        Ok(package::normalize_symbol_name(name))
    }

    pub(in crate::evaluator) fn name_designator_from_value(
        value: &Value,
        span: Span,
    ) -> Result<String, RuntimeError> {
        let raw = match value {
            Value::String(name) => name.as_ref(),
            _ => value.symbol_name().ok_or_else(|| RuntimeError::Type {
                expected: "SYMBOL DESIGNATOR".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            })?,
        };
        let name = raw.strip_prefix(':').unwrap_or(raw);
        if name.is_empty() {
            return Err(Self::invalid("symbol name cannot be empty", span));
        }
        Ok(unqualified_name(name))
    }

    pub(in crate::evaluator) fn slot_name_from_value(
        value: &Value,
        span: Span,
    ) -> Result<String, RuntimeError> {
        Self::name_designator_from_value(value, span)
    }
}

#[cfg(test)]
mod tests {
    use ncl_syntax::Span;

    use crate::{Runtime, Value};

    const SPAN: Span = Span::new(0, 1);

    fn valid<T, E>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|_| panic!("expected a valid package or symbol designator"))
    }

    #[test]
    fn value_name_helpers_cover_designators_and_errors() {
        let span = SPAN;
        let package_cases = [
            (Value::Package("user".into()), "USER"),
            (Value::String("common-lisp".into()), "COMMON-LISP"),
            (Value::symbol("keyword"), "KEYWORD"),
        ];
        for (value, expected) in package_cases {
            assert_eq!(
                valid(Runtime::package_designator_name(&value, span)),
                expected
            );
        }
        assert!(Runtime::package_designator_name(&Value::Integer(1), span).is_err());
        assert!(Runtime::package_designator_name(&Value::symbol("foo:bar"), span).is_err());

        let symbol_cases = [
            (Value::String(":name".into()), "NAME"),
            (Value::symbol("name"), "NAME"),
            (Value::keyword("key"), "KEY"),
        ];
        for (value, expected) in symbol_cases {
            assert_eq!(
                valid(Runtime::symbol_name_from_value(&value, span)),
                expected
            );
            assert_eq!(
                valid(Runtime::name_designator_from_value(&value, span)),
                expected
            );
        }
        assert!(Runtime::symbol_name_from_value(&Value::Integer(1), span).is_err());
        assert!(Runtime::name_designator_from_value(&Value::String(":".into()), span).is_err());
    }
}
