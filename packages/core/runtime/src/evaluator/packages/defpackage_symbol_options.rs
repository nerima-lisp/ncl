use ncl_syntax::{Form, Span};

use crate::{Runtime, RuntimeError};

use super::defpackage_types::{DefpackageBuilder, DefpackageOperation};

impl Runtime {
    /// Applies a `DEFPACKAGE` option that names symbols to export, shadow,
    /// intern, or import (`:export`, `:shadow`, `:intern`, `:import-from`,
    /// `:shadowing-import-from`). Returns `Ok(false)` when
    /// `normalized_option` names a different kind of option, so the caller
    /// can try the next handler.
    pub(super) fn apply_defpackage_symbol_option(
        builder: &mut DefpackageBuilder,
        normalized_option: &str,
        option_items: &[Form],
        option_span: Span,
    ) -> Result<bool, RuntimeError> {
        match normalized_option {
            "EXPORT" => {
                for symbol_form in option_items.iter().skip(1) {
                    builder
                        .exports
                        .insert(Self::symbol_name_from_form(symbol_form)?);
                }
            }
            "SHADOW" => {
                for symbol_form in option_items.iter().skip(1) {
                    builder.operations.push(DefpackageOperation::Shadow(
                        Self::symbol_name_from_form(symbol_form)?,
                    ));
                }
            }
            "INTERN" => {
                for symbol_form in option_items.iter().skip(1) {
                    builder.operations.push(DefpackageOperation::Intern(
                        Self::symbol_name_from_form(symbol_form)?,
                    ));
                }
            }
            "IMPORT-FROM" | "SHADOWING-IMPORT-FROM" => {
                if option_items.len() < 2 {
                    return Err(Self::invalid(
                        "defpackage import option needs a package name",
                        option_span,
                    ));
                }
                let source_package = Self::package_name_from_form(&option_items[1])?;
                let shadowing = normalized_option == "SHADOWING-IMPORT-FROM";
                for symbol_form in option_items.iter().skip(2) {
                    builder.operations.push(DefpackageOperation::Import {
                        source_package: source_package.clone(),
                        source_name: Self::symbol_name_from_form(symbol_form)?,
                        shadowing,
                    });
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
