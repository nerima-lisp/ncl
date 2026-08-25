use ncl_syntax::{Form, FormKind, Span, SymbolTokenKind, parse_symbol_token};

use crate::environment::normalize_name;
use crate::package;
use crate::{Runtime, RuntimeError};

use super::helpers::{atom_name, is_special_form, literal_atom};

impl Runtime {
    pub(super) fn resolve_form(&self, form: &Form) -> Result<Form, RuntimeError> {
        let current = self.current_package();
        self.resolve_form_in(form, &current)
    }

    fn resolve_form_in(&self, form: &Form, current: &str) -> Result<Form, RuntimeError> {
        let kind = match &form.kind {
            FormKind::Atom(atom) => {
                let escaped = parse_symbol_token(atom)
                    .map(|token| token.escaped)
                    .unwrap_or(false);
                if escaped {
                    FormKind::Atom(atom.clone())
                } else {
                    FormKind::Atom(self.resolve_atom(atom, current, form.span)?)
                }
            }
            FormKind::String(value) => FormKind::String(value.clone()),
            FormKind::Character(value) => FormKind::Character(*value),
            FormKind::List(items) => {
                let mut resolved = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    if index == 0 && is_special_form(item) {
                        resolved.push(Form::atom(
                            normalize_name(atom_name(item).unwrap_or_default()),
                            item.span,
                        ));
                    } else {
                        resolved.push(self.resolve_form_in(item, current)?);
                    }
                }
                FormKind::List(resolved)
            }
            FormKind::DottedList { items, tail } => FormKind::DottedList {
                items: items
                    .iter()
                    .map(|item| self.resolve_form_in(item, current))
                    .collect::<Result<Vec<_>, _>>()?,
                tail: Box::new(self.resolve_form_in(tail, current)?),
            },
            FormKind::Vector(items) => FormKind::Vector(
                items
                    .iter()
                    .map(|item| self.resolve_form_in(item, current))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        };
        Ok(Form::new(kind, form.span))
    }

    fn resolve_atom(&self, atom: &str, current: &str, span: Span) -> Result<String, RuntimeError> {
        let token =
            parse_symbol_token(atom).map_err(|_| self.package_error("invalid symbol", span))?;
        match token.kind {
            SymbolTokenKind::Uninterned => return Ok(format!("#:{}", token.name)),
            SymbolTokenKind::Keyword => return Ok(format!(":{}", token.name)),
            SymbolTokenKind::Symbol => {}
        }
        if token.package.is_none()
            && !token.escaped
            && (literal_atom(atom).is_some() || token.name.starts_with('&'))
        {
            return Ok(normalize_name(&token.name));
        }
        if let Some(package_name) = token.package.as_deref() {
            let package_name = package::normalize_package_name(package_name);
            let symbol_name = normalize_name(&token.name);
            if package_name.is_empty() || symbol_name.is_empty() {
                return Err(self.package_error("invalid package-qualified symbol", span));
            }
            let package_name = {
                let packages = self.packages.borrow();
                let package_name = packages.canonical_package_name_for(current, &package_name);
                if !packages.package_exists(&package_name) {
                    return Err(
                        self.package_error(&format!("unknown package {package_name}"), span)
                    );
                }
                if token.external && !packages.is_exported(&package_name, &symbol_name) {
                    return Err(self.package_error(
                        &format!(
                            "symbol {symbol_name} is not exported from package {package_name}"
                        ),
                        span,
                    ));
                }
                package_name
            };
            self.packages
                .borrow_mut()
                .ensure_symbol(&package_name, &symbol_name);
            return Ok(package::canonical_symbol_name(&package_name, &symbol_name));
        }
        let normalized = normalize_name(&token.name);
        let package_name = if current == package::DEFAULT_PACKAGE {
            package::DEFAULT_PACKAGE.to_string()
        } else {
            current.to_string()
        };
        self.packages
            .borrow_mut()
            .ensure_symbol(&package_name, &normalized);
        Ok(package::canonical_symbol_name(&package_name, &normalized))
    }
}
