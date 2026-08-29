use ncl_syntax::Span;

use crate::{Runtime, RuntimeError, Value, package};

impl Runtime {
    pub(in crate::evaluator) fn package_names_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<String>, RuntimeError> {
        let values = value
            .list_items()
            .ok_or_else(|| Self::invalid("package designators must be a proper list", span))?;
        values
            .iter()
            .map(|value| self.package_name_from_value(value, span))
            .collect()
    }

    pub(in crate::evaluator) fn symbol_names_from_value(
        value: &Value,
        span: Span,
    ) -> Result<Vec<String>, RuntimeError> {
        let values = value
            .list_items()
            .ok_or_else(|| Self::invalid("symbol designators must be a proper list", span))?;
        values
            .iter()
            .map(|value| Self::symbol_name_from_value(value, span))
            .collect()
    }

    pub(in crate::evaluator) fn symbol_import_references_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<(String, String)>, RuntimeError> {
        let values = value
            .list_items()
            .ok_or_else(|| Self::invalid("symbol designators must be a proper list", span))?;
        values
            .iter()
            .map(|value| {
                if matches!(value, Value::UninternedSymbol(_)) {
                    return Err(Self::invalid("uninterned symbols cannot be imported", span));
                }
                let raw = value.symbol_name().ok_or_else(|| RuntimeError::Type {
                    expected: "SYMBOL".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                })?;
                if matches!(value, Value::Keyword(_) | Value::KeywordExact(_)) {
                    return Ok((
                        package::KEYWORD_PACKAGE.to_string(),
                        package::normalize_symbol_name(raw),
                    ));
                }
                if let Some((package_name, symbol_name, _)) = package::split_symbol(raw) {
                    return Ok((
                        package::normalize_package_name(package_name),
                        package::normalize_symbol_name(symbol_name),
                    ));
                }
                Ok((self.current_package(), package::normalize_symbol_name(raw)))
            })
            .collect()
    }

    pub(in crate::evaluator) fn package_symbol_value(
        &self,
        package_name: &str,
        symbol_name: &str,
    ) -> Value {
        let package_name = self.packages.borrow().canonical_package_name(package_name);
        if package_name == package::KEYWORD_PACKAGE {
            Value::keyword(symbol_name)
        } else {
            let symbol_name = self
                .packages
                .borrow()
                .imported_symbol_name(&package_name, symbol_name);
            Value::symbol(symbol_name)
        }
    }

    pub(in crate::evaluator) fn symbol_status_value(status: package::SymbolStatus) -> Value {
        match status {
            package::SymbolStatus::Internal => Value::keyword("INTERNAL"),
            package::SymbolStatus::External => Value::keyword("EXTERNAL"),
        }
    }
}

#[cfg(test)]
mod tests {
    use ncl_syntax::Span;

    use crate::{Runtime, Value};

    const SPAN: Span = Span::new(0, 1);

    fn valid<T, E>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|_| panic!("expected a valid designator list"))
    }

    #[test]
    fn package_and_symbol_lists_are_table_driven() {
        let runtime = Runtime::new();
        let packages = Value::list(vec![
            Value::String("ncl-user".into()),
            Value::symbol("keyword"),
        ]);
        assert_eq!(
            valid(runtime.package_names_from_value(&packages, SPAN)),
            ["NCL-USER", "KEYWORD"]
        );

        let symbols = Value::list(vec![Value::symbol("one"), Value::keyword("two")]);
        assert_eq!(
            valid(Runtime::symbol_names_from_value(&symbols, SPAN)),
            ["ONE", "TWO"]
        );

        let invalid = Value::Integer(1);
        assert!(runtime.package_names_from_value(&invalid, SPAN).is_err());
        assert!(Runtime::symbol_names_from_value(&invalid, SPAN).is_err());
    }

    #[test]
    fn import_references_resolve_keyword_qualified_and_current_symbols() {
        let runtime = Runtime::new();
        let references = Value::list(vec![
            Value::keyword("key"),
            Value::symbol("common-lisp:car"),
            Value::symbol("local"),
        ]);
        assert_eq!(
            valid(runtime.symbol_import_references_from_value(&references, SPAN)),
            [
                ("KEYWORD".into(), "KEY".into()),
                ("COMMON-LISP".into(), "CAR".into()),
                ("NCL-USER".into(), "LOCAL".into())
            ]
        );
        assert!(
            runtime
                .symbol_import_references_from_value(&Value::Integer(1), SPAN)
                .is_err()
        );
        assert!(
            runtime
                .symbol_import_references_from_value(
                    &Value::list(vec![Value::UninternedSymbol("x".into())]),
                    SPAN
                )
                .is_err()
        );
    }
}
