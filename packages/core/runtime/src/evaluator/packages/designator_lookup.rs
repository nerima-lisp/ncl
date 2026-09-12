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

    pub(in crate::evaluator) fn package_nickname_names_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<String>, RuntimeError> {
        let values = value
            .list_items()
            .ok_or_else(|| Self::invalid("package nicknames must be a proper list", span))?;
        values
            .iter()
            .map(|value| Self::package_designator_name(value, span))
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

    pub(in crate::evaluator) fn symbol_references_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<(String, bool)>, RuntimeError> {
        let values = if matches!(value, Value::Nil | Value::Boolean(false)) {
            vec![value.clone()]
        } else {
            value.list_items().unwrap_or_else(|| vec![value.clone()])
        };
        values
            .iter()
            .map(|value| {
                let (raw, exact) = match value {
                    Value::String(name) => (name.to_string(), false),
                    value => value.symbol_reference().ok_or_else(|| RuntimeError::Type {
                        expected: "SYMBOL".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(span),
                    })?,
                };
                let raw = raw.strip_prefix(':').unwrap_or(&raw);
                if exact {
                    let name = package::split_symbol(raw)
                        .map(|(_, symbol_name, _)| symbol_name)
                        .unwrap_or(raw);
                    if name.is_empty() || name.contains(':') {
                        return Err(Self::invalid(
                            "qualified symbol designators are not supported here",
                            span,
                        ));
                    }
                    Ok((name.to_string(), true))
                } else {
                    Ok((Self::symbol_name_from_value(value, span)?, false))
                }
            })
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
                if let Value::InternedSymbol(symbol) = value {
                    let package_name = symbol.package().name().ok_or_else(|| {
                        Self::invalid("symbols from deleted packages cannot be imported", span)
                    })?;
                    return Ok((
                        package::normalize_package_name(&package_name),
                        package::normalize_symbol_name(symbol.name()),
                    ));
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
        let (package_name, symbol_object, exact_name, imported_name) = {
            let packages = self.packages.borrow();
            let package_name = packages.canonical_package_name(package_name);
            let symbol_object = packages.symbol_object_for(&package_name, symbol_name);
            let exact_name = packages.exact_symbol_name(&package_name, symbol_name);
            let imported_name = packages.imported_symbol_name(&package_name, symbol_name);
            (package_name, symbol_object, exact_name, imported_name)
        };
        if let Some(symbol_object) = symbol_object {
            return Value::interned_symbol(symbol_object);
        }
        if let Some(exact_name) = exact_name {
            let exact_name = package::canonical_exact_symbol_name(&package_name, &exact_name);
            return if package_name == package::KEYWORD_PACKAGE {
                Value::keyword_exact(exact_name)
            } else {
                Value::symbol_exact(exact_name)
            };
        }
        if package_name == package::KEYWORD_PACKAGE {
            Value::keyword(symbol_name)
        } else {
            Value::symbol(imported_name)
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
