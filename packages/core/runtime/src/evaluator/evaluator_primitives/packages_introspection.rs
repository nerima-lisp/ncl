#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_package_introspection_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(
            name,
            "FIND-PACKAGE"
                | "PACKAGE-NAME"
                | "PACKAGE-NICKNAMES"
                | "PACKAGE-LOCAL-NICKNAMES"
                | "PACKAGE-LOCALLY-NICKNAMED-BY-LIST"
                | "PACKAGE-SHADOWING-SYMBOLS"
                | "PACKAGE-USE-LIST"
                | "PACKAGE-USED-BY-LIST"
                | "SYMBOL-PACKAGE"
                | "__NCL-ALL-SYMBOLS"
                | "__NCL-PACKAGE-SYMBOLS"
        ) {
            return None;
        }
        Some((|| -> Result<Value, RuntimeError> {
            match name {
                "__NCL-ALL-SYMBOLS" => {
                    if !arguments.is_empty() {
                        return Err(Self::arity("__ncl-all-symbols", "zero", arguments.len()));
                    }
                    Ok(Value::list(
                        self.packages
                            .borrow()
                            .all_symbol_objects()
                            .into_iter()
                            .map(Value::interned_symbol)
                            .collect(),
                    ))
                }
                "__NCL-PACKAGE-SYMBOLS" => {
                    if arguments.len() != 2 {
                        return Err(Self::arity("__ncl-package-symbols", "two", arguments.len()));
                    }
                    let package = self.package_name_from_value(&arguments[0], span)?;
                    let external_only = arguments[1].is_truthy();
                    Ok(Value::list(
                        self.packages
                            .borrow()
                            .package_symbol_objects_for(&package, external_only)
                            .into_iter()
                            .map(Value::interned_symbol)
                            .collect(),
                    ))
                }
                "SYMBOL-PACKAGE" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("symbol-package", "one", arguments.len()));
                    }
                    let package = match &arguments[0] {
                        Value::InternedSymbol(symbol) => {
                            let packages = self.packages.borrow();
                            return Ok(symbol
                                .package()
                                .name()
                                .and_then(|name| packages.package_object_for(&name))
                                .map(Value::package_object)
                                .unwrap_or(Value::Nil));
                        }
                        Value::UninternedSymbol(_) => return Ok(Value::Nil),
                        Value::Keyword(_) | Value::KeywordExact(_) => {
                            package::KEYWORD_PACKAGE.to_string()
                        }
                        Value::Nil | Value::Boolean(_) => package::COMMON_LISP_PACKAGE.to_string(),
                        Value::Symbol(symbol) | Value::SymbolExact(symbol) => {
                            package::split_symbol(symbol.as_ref())
                                .map(|(name, _, _)| package::normalize_package_name(name))
                                .unwrap_or_else(|| package::DEFAULT_PACKAGE.to_string())
                        }
                        value => {
                            return Err(RuntimeError::Type {
                                expected: "SYMBOL".into(),
                                actual: value.type_name().into(),
                                span: Some(span),
                            });
                        }
                    };
                    let packages = self.packages.borrow();
                    Ok(packages
                        .package_object_for(&package)
                        .map(Value::package_object)
                        .unwrap_or(Value::Nil))
                }
                "FIND-PACKAGE" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("find-package", "one", arguments.len()));
                    }
                    let package = Self::package_designator_name(&arguments[0], span)?;
                    let packages = self.packages.borrow();
                    Ok(packages
                        .package_object_for(&package)
                        .map(Value::package_object)
                        .unwrap_or(Value::Nil))
                }
                "PACKAGE-NAME" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("package-name", "one", arguments.len()));
                    }
                    match &arguments[0] {
                        Value::Package(package) => Ok(Value::string(package.as_ref())),
                        Value::PackageObject(package) => Ok(package
                            .name()
                            .map_or(Value::Nil, |name| Value::string(name.as_str()))),
                        other => Err(RuntimeError::Type {
                            expected: "PACKAGE".into(),
                            actual: other.type_name().into(),
                            span: Some(span),
                        }),
                    }
                }
                "PACKAGE-NICKNAMES" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("package-nicknames", "one", arguments.len()));
                    }
                    let package = self.package_name_from_value(&arguments[0], span)?;
                    Ok(Value::list(
                        self.packages
                            .borrow()
                            .package_nicknames_for(&package)
                            .into_iter()
                            .map(Value::string)
                            .collect(),
                    ))
                }
                "PACKAGE-LOCAL-NICKNAMES" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity(
                            "package-local-nicknames",
                            "one",
                            arguments.len(),
                        ));
                    }
                    let package = self.package_name_from_value(&arguments[0], span)?;
                    let packages = self.packages.borrow();
                    Ok(Value::list(
                        packages
                            .package_local_nicknames_for(&package)
                            .into_iter()
                            .map(|(nickname, target)| {
                                Value::cons(
                                    Value::string(nickname),
                                    packages
                                        .package_object_for(&target)
                                        .map(Value::package_object)
                                        .unwrap_or(Value::Nil),
                                )
                            })
                            .collect(),
                    ))
                }
                "PACKAGE-LOCALLY-NICKNAMED-BY-LIST" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity(
                            "package-locally-nicknamed-by-list",
                            "one",
                            arguments.len(),
                        ));
                    }
                    let package = self.package_name_from_value(&arguments[0], span)?;
                    let packages = self.packages.borrow();
                    Ok(Value::list(
                        packages
                            .package_locally_nicknamed_by_list_for(&package)
                            .into_iter()
                            .filter_map(|name| {
                                packages
                                    .package_object_for(&name)
                                    .map(Value::package_object)
                            })
                            .collect(),
                    ))
                }
                "PACKAGE-SHADOWING-SYMBOLS" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity(
                            "package-shadowing-symbols",
                            "one",
                            arguments.len(),
                        ));
                    }
                    let package = self.package_name_from_value(&arguments[0], span)?;
                    Ok(Value::list(
                        self.packages
                            .borrow()
                            .package_shadowing_symbols_for(&package)
                            .into_iter()
                            .map(Value::symbol)
                            .collect(),
                    ))
                }
                "PACKAGE-USE-LIST" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("package-use-list", "one", arguments.len()));
                    }
                    let package = self.package_name_from_value(&arguments[0], span)?;
                    let packages = self.packages.borrow();
                    Ok(Value::list(
                        packages
                            .use_packages_for(&package)
                            .into_iter()
                            .filter_map(|name| {
                                packages
                                    .package_object_for(&name)
                                    .map(Value::package_object)
                            })
                            .collect(),
                    ))
                }
                "PACKAGE-USED-BY-LIST" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("package-used-by-list", "one", arguments.len()));
                    }
                    let package = self.package_name_from_value(&arguments[0], span)?;
                    let packages = self.packages.borrow();
                    Ok(Value::list(
                        packages
                            .package_used_by_list_for(&package)
                            .into_iter()
                            .filter_map(|name| {
                                packages
                                    .package_object_for(&name)
                                    .map(Value::package_object)
                            })
                            .collect(),
                    ))
                }
                _ => unreachable!("package introspection primitive name was prevalidated"),
            }
        })())
    }
}
