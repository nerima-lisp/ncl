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
mod tests;
