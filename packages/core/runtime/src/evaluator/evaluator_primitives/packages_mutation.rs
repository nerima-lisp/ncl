#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_package_use_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(
            name,
            "USE-PACKAGE" | "UNUSE-PACKAGE" | "EXPORT" | "UNEXPORT"
        ) {
            return None;
        }
        let result = (|| -> Result<Value, RuntimeError> {
            if arguments.len() != 1 && arguments.len() != 2 {
                return Err(Self::arity(name, "one or two", arguments.len()));
            }
            let target = arguments
                .get(1)
                .map(|value| self.package_name_from_value(value, span))
                .transpose()?
                .unwrap_or_else(|| self.current_package());
            match name {
                "USE-PACKAGE" | "UNUSE-PACKAGE" => {
                    let packages = self.package_names_from_value(&arguments[0], span)?;
                    if name == "USE-PACKAGE" && packages.iter().any(|package| package == &target) {
                        return Err(Self::package_error("a package cannot use itself", span));
                    }
                    let mut state = self.packages.borrow_mut();
                    for package in packages {
                        match name {
                            "USE-PACKAGE" => state.use_package(&package, &target),
                            "UNUSE-PACKAGE" => state.unuse_package(&package, &target),
                            _ => unreachable!("package use primitive name was prevalidated"),
                        }
                    }
                    Ok(Value::boolean(true))
                }
                "EXPORT" | "UNEXPORT" => {
                    let symbols = Self::symbol_names_from_value(&arguments[0], span)?;
                    let mut state = self.packages.borrow_mut();
                    if name == "EXPORT" {
                        state.export_symbols(&target, &symbols);
                    } else {
                        state.unexport_symbols(&target, &symbols);
                    }
                    Ok(Value::boolean(true))
                }
                _ => unreachable!("package use primitive name was prevalidated"),
            }
        })();
        Some(result)
    }

    pub(crate) fn apply_package_symbol_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "IMPORT" | "SHADOWING-IMPORT" | "SHADOW" | "UNINTERN") {
            return None;
        }
        let result = (|| -> Result<Value, RuntimeError> {
            if arguments.len() != 1 && arguments.len() != 2 {
                return Err(Self::arity(name, "one or two", arguments.len()));
            }
            let target = arguments
                .get(1)
                .map(|value| self.package_name_from_value(value, span))
                .transpose()?
                .unwrap_or_else(|| self.current_package());
            match name {
                "IMPORT" | "SHADOWING-IMPORT" => {
                    let imports = self.symbol_import_references_from_value(&arguments[0], span)?;
                    {
                        let state = self.packages.borrow();
                        for (source_package, source_name) in &imports {
                            if !state.symbol_exists(source_package, source_name) {
                                return Err(Self::package_error(
                                    &format!("unknown symbol {source_package}::{source_name}"),
                                    span,
                                ));
                            }
                        }
                    }
                    let shadowing = name == "SHADOWING-IMPORT";
                    let mut state = self.packages.borrow_mut();
                    for (source_package, source_name) in imports {
                        state.import_symbol(&source_package, &source_name, &target, shadowing);
                    }
                    Ok(Value::boolean(true))
                }
                "SHADOW" => {
                    let symbols = Self::symbol_names_from_value(&arguments[0], span)?;
                    let mut state = self.packages.borrow_mut();
                    for symbol in symbols {
                        state.shadow_symbol(&target, &symbol);
                    }
                    Ok(Value::boolean(true))
                }
                "UNINTERN" => {
                    let symbols = Self::symbol_names_from_value(&arguments[0], span)?;
                    let mut removed = false;
                    let mut local_names = Vec::new();
                    {
                        let mut state = self.packages.borrow_mut();
                        for symbol in symbols {
                            let local_name = package::canonical_symbol_name(&target, &symbol);
                            removed |= state.unintern_symbol(&target, &symbol);
                            local_names.push(local_name);
                        }
                    }
                    for local_name in local_names {
                        self.remove_global_symbol(&local_name);
                    }
                    Ok(Value::boolean(removed))
                }
                _ => unreachable!("package symbol primitive name was prevalidated"),
            }
        })();
        Some(result)
    }
}
