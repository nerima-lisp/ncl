use ncl_syntax::Span;

use super::helpers::unqualified_name;
use crate::package;
use crate::{Runtime, RuntimeError, Value};

impl Runtime {
    pub(super) fn package_designator_name(
        &self,
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
            return Err(self.package_error("package name cannot be qualified", span));
        }
        let name = package::normalize_package_name(raw);
        if name.is_empty() || name.contains(':') {
            return Err(self.package_error("invalid package name", span));
        }
        Ok(name)
    }

    pub(super) fn package_name_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<String, RuntimeError> {
        let name = self.package_designator_name(value, span)?;
        let packages = self.packages.borrow();
        if !packages.package_exists(&name) {
            return Err(self.package_error(&format!("unknown package {name}"), span));
        }
        Ok(packages.canonical_package_name(&name))
    }

    pub(super) fn symbol_name_from_value(
        &self,
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
            return Err(self.package_error("symbol name cannot be qualified", span));
        }
        Ok(package::normalize_symbol_name(name))
    }

    pub(super) fn name_designator_from_value(
        &self,
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
            return Err(self.invalid("symbol name cannot be empty", span));
        }
        Ok(unqualified_name(name))
    }

    pub(super) fn slot_name_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<String, RuntimeError> {
        self.name_designator_from_value(value, span)
    }

    pub(super) fn package_names_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<String>, RuntimeError> {
        let values = value
            .list_items()
            .ok_or_else(|| self.invalid("package designators must be a proper list", span))?;
        values
            .iter()
            .map(|value| self.package_name_from_value(value, span))
            .collect()
    }

    pub(super) fn symbol_names_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<String>, RuntimeError> {
        let values = value
            .list_items()
            .ok_or_else(|| self.invalid("symbol designators must be a proper list", span))?;
        values
            .iter()
            .map(|value| self.symbol_name_from_value(value, span))
            .collect()
    }

    pub(super) fn symbol_import_references_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<(String, String)>, RuntimeError> {
        let values = value
            .list_items()
            .ok_or_else(|| self.invalid("symbol designators must be a proper list", span))?;
        values
            .iter()
            .map(|value| {
                if matches!(value, Value::UninternedSymbol(_)) {
                    return Err(self.invalid("uninterned symbols cannot be imported", span));
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

    pub(super) fn package_symbol_value(&self, package_name: &str, symbol_name: &str) -> Value {
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

    pub(super) fn symbol_status_value(status: package::SymbolStatus) -> Value {
        match status {
            package::SymbolStatus::Internal => Value::keyword("INTERNAL"),
            package::SymbolStatus::External => Value::keyword("EXTERNAL"),
        }
    }
}
