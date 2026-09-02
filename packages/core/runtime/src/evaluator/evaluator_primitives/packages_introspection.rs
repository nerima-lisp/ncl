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
                | "PACKAGE-USE-LIST"
                | "PACKAGE-NICKNAMES"
                | "PACKAGE-SHADOWING-SYMBOLS"
                | "PACKAGE-USED-BY-LIST"
                | "FIND-ALL-SYMBOLS"
                | "__NCL-PACKAGE-SYMBOLS"
                | "__NCL-ALL-SYMBOLS"
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
                            .all_symbols()
                            .into_iter()
                            .map(Value::symbol)
                            .collect(),
                    ))
                }
                "__NCL-PACKAGE-SYMBOLS" => {
                    if arguments.len() != 2 {
                        return Err(Self::arity("__ncl-package-symbols", "two", arguments.len()));
                    }
                    let package = Self::package_designator_name(&arguments[0], span)?;
                    Ok(Value::list(
                        self.packages
                            .borrow()
                            .symbols_for(&package, arguments[1].is_truthy())
                            .into_iter()
                            .map(Value::symbol)
                            .collect(),
                    ))
                }
                "FIND-ALL-SYMBOLS" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("find-all-symbols", "one", arguments.len()));
                    }
                    let (Value::Symbol(symbol) | Value::SymbolExact(symbol)) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "SYMBOL".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let symbols = self.packages.borrow().symbols_named(symbol);
                    Ok(Value::list(symbols.into_iter().map(|(package, _)| {
                        Value::symbol(package::canonical_symbol_name(&package, symbol))
                    }).collect()))
                }
                "FIND-PACKAGE" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("find-package", "one", arguments.len()));
                    }
                    let package = Self::package_designator_name(&arguments[0], span)?;
                    let packages = self.packages.borrow();
                    Ok(if packages.package_exists(&package) {
                        Value::package(packages.canonical_package_name(&package))
                    } else {
                        Value::Nil
                    })
                }
                "PACKAGE-NAME" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("package-name", "one", arguments.len()));
                    }
                    match &arguments[0] {
                        Value::Package(package) => Ok(Value::string(package.as_ref())),
                        other => Err(RuntimeError::Type {
                            expected: "PACKAGE".into(),
                            actual: other.type_name().into(),
                            span: Some(span),
                        }),
                    }
                }
                "PACKAGE-USE-LIST" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("package-use-list", "one", arguments.len()));
                    }
                    match &arguments[0] {
                        Value::Package(package) => Ok(Value::list(
                            self.packages
                                .borrow()
                                .use_packages_for(package)
                                .into_iter()
                                .map(Value::package)
                                .collect(),
                        )),
                        other => Err(RuntimeError::Type {
                            expected: "PACKAGE".into(),
                            actual: other.type_name().into(),
                            span: Some(span),
                        }),
                    }
                }
                "PACKAGE-NICKNAMES" | "PACKAGE-SHADOWING-SYMBOLS" | "PACKAGE-USED-BY-LIST" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity(&name.to_lowercase(), "one", arguments.len()));
                    }
                    let package = match &arguments[0] {
                        Value::Package(package) => package,
                        other => {
                            return Err(RuntimeError::Type {
                                expected: "PACKAGE".into(),
                                actual: other.type_name().into(),
                                span: Some(span),
                            });
                        }
                    };
                    let values = {
                        let packages = self.packages.borrow();
                        match name {
                            "PACKAGE-NICKNAMES" => packages
                                .package_nicknames(package)
                                .into_iter()
                                .map(Value::string)
                                .collect(),
                            "PACKAGE-SHADOWING-SYMBOLS" => packages
                                .shadowing_symbols_for(package)
                                .into_iter()
                                .map(|symbol| {
                                    Value::symbol(package::canonical_symbol_name(
                                        package, &symbol,
                                    ))
                                })
                                .collect(),
                            "PACKAGE-USED-BY-LIST" => packages
                                .packages_using(package)
                                .into_iter()
                                .map(Value::package)
                                .collect(),
                            _ => unreachable!("package introspection primitive was prevalidated"),
                        }
                    };
                    Ok(Value::list(values))
                }
                _ => unreachable!("package introspection primitive name was prevalidated"),
            }
        })())
    }
}
