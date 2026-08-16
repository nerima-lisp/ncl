impl Runtime {
    fn apply_package_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match name {
            "MAKE-PACKAGE" => self.make_package_from_arguments(arguments, span),
            "INTERN" => {
                if !(1..=2).contains(&arguments.len()) {
                    return Err(self.arity("intern", "one or two", arguments.len()));
                }
                let symbol_name = self.symbol_name_from_value(&arguments[0], span)?;
                let package_name = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let status = match self
                    .packages
                    .borrow_mut()
                    .intern_symbol(&package_name, &symbol_name)
                {
                    Some(status) => status,
                    None => {
                        return Err(
                            self.package_error(&format!("unknown package {package_name}"), span)
                        );
                    }
                };
                let symbol = self.package_symbol_value(&package_name, &symbol_name);
                Ok(Value::values(vec![
                    symbol,
                    Self::symbol_status_value(status),
                ]))
            }
            "FIND-SYMBOL" => {
                if !(1..=2).contains(&arguments.len()) {
                    return Err(self.arity("find-symbol", "one or two", arguments.len()));
                }
                let symbol_name = self.symbol_name_from_value(&arguments[0], span)?;
                let package_name = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let status = self
                    .packages
                    .borrow()
                    .symbol_status(&package_name, &symbol_name);
                match status {
                    Some(status) => {
                        let symbol = self.package_symbol_value(&package_name, &symbol_name);
                        Ok(Value::values(vec![
                            symbol,
                            Self::symbol_status_value(status),
                        ]))
                    }
                    None => Ok(Value::values(vec![Value::Nil, Value::Nil])),
                }
            }
            "FIND-PACKAGE" => {
                if arguments.len() != 1 {
                    return Err(self.arity("find-package", "one", arguments.len()));
                }
                let package = self.package_designator_name(&arguments[0], span)?;
                let packages = self.packages.borrow();
                if packages.package_exists(&package) {
                    Ok(Value::package(packages.canonical_package_name(&package)))
                } else {
                    Ok(Value::Nil)
                }
            }
            "DELETE-PACKAGE" => {
                if arguments.len() != 1 {
                    return Err(self.arity("delete-package", "one", arguments.len()));
                }
                let package = self.package_name_from_value(&arguments[0], span)?;
                let deleted = self
                    .packages
                    .borrow_mut()
                    .delete_package(&package)
                    .map_err(|message| self.package_error(&message, span))?;
                Ok(Value::boolean(deleted))
            }
            "RENAME-PACKAGE" => {
                if !(2..=3).contains(&arguments.len()) {
                    return Err(self.arity("rename-package", "two or three", arguments.len()));
                }
                let package = self.package_name_from_value(&arguments[0], span)?;
                let new_name = self.name_designator_from_value(&arguments[1], span)?;
                let nicknames = arguments
                    .get(2)
                    .map(|value| self.package_nicknames_from_value(value, span))
                    .transpose()?
                    .unwrap_or_default();
                let name = self
                    .packages
                    .borrow_mut()
                    .rename_package(&package, new_name, nicknames)
                    .map_err(|message| self.package_error(&message, span))?;
                Ok(Value::package(name))
            }
            "PACKAGE-NAME" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-name", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => Ok(Value::string(package.as_ref())),
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "PACKAGE-USE-LIST" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-use-list", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => {
                        let names = self.packages.borrow().use_packages_for(package);
                        Ok(Value::list(names.into_iter().map(Value::package).collect()))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "PACKAGE-NICKNAMES" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-nicknames", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => {
                        let nicknames = self.packages.borrow().package_nicknames(package);
                        Ok(Value::list(
                            nicknames.into_iter().map(Value::string).collect(),
                        ))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "PACKAGE-SHADOWING-SYMBOLS" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-shadowing-symbols", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => {
                        let symbols = self
                            .packages
                            .borrow()
                            .shadowing_symbols_for(package)
                            .into_iter()
                            .map(|symbol| {
                                self.package_symbol_value(symbol.package(), symbol.name())
                            })
                            .collect();
                        Ok(Value::list(symbols))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "PACKAGE-USED-BY-LIST" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-used-by-list", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => {
                        let packages = self.packages.borrow().used_by_packages_for(package);
                        Ok(Value::list(
                            packages.into_iter().map(Value::package).collect(),
                        ))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "DOCUMENTATION" => {
                if arguments.len() != 2 {
                    return Err(self.arity("documentation", "two", arguments.len()));
                }
                match &arguments[0] {
                    Value::Class(class) => {
                        let documentation = class.documentation.borrow();
                        Ok(documentation.as_ref().map_or(Value::Nil, |documentation| {
                            Value::string(documentation.as_str())
                        }))
                    }
                    Value::Package(package) => Ok(self
                        .packages
                        .borrow()
                        .package_documentation(package)
                        .map_or(Value::Nil, |documentation| {
                            Value::string(documentation.as_str())
                        })),
                    other if other.symbol_reference().is_some() => {
                        let (name, exact) = other.symbol_reference().expect("symbol reference");
                        let (doc_type, _) = arguments[1].symbol_reference().ok_or_else(|| {
                            self.invalid("documentation type must be a symbol", span)
                        })?;
                        let documentation = match unqualified_name(doc_type).as_str() {
                            "FUNCTION" => {
                                if exact {
                                    environment.lookup_function_documentation_exact(name)
                                } else {
                                    environment.lookup_function_documentation(name)
                                }
                            }
                            "VARIABLE" => {
                                if exact {
                                    environment.lookup_variable_documentation_exact(name)
                                } else {
                                    environment.lookup_variable_documentation(name)
                                }
                            }
                            _ => None,
                        };
                        Ok(documentation.map_or(Value::Nil, |documentation| {
                            Value::string(documentation.as_str())
                        }))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "CLASS, PACKAGE, or SYMBOL".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "LIST-ALL-PACKAGES" => {
                if !arguments.is_empty() {
                    return Err(self.arity("list-all-packages", "zero", arguments.len()));
                }
                let names = self.packages.borrow().all_package_names();
                Ok(Value::list(names.into_iter().map(Value::package).collect()))
            }
            "USE-PACKAGE" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("use-package", "one or two", arguments.len()));
                }
                let packages = self.package_names_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                if packages.iter().any(|package| package == &target) {
                    return Err(self.package_error("a package cannot use itself", span));
                }
                let mut state = self.packages.borrow_mut();
                for package in packages {
                    state
                        .use_package(&package, &target)
                        .map_err(|message| self.package_error(&message, span))?;
                }
                Ok(Value::boolean(true))
            }
            "UNUSE-PACKAGE" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("unuse-package", "one or two", arguments.len()));
                }
                let packages = self.package_names_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let mut state = self.packages.borrow_mut();
                for package in packages {
                    state.unuse_package(&package, &target);
                }
                Ok(Value::boolean(true))
            }
            "EXPORT" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("export", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value(&arguments[0], span)?;
                let package = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                self.packages
                    .borrow_mut()
                    .export_symbols(&package, &symbols);
                Ok(Value::boolean(true))
            }
            "UNEXPORT" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("unexport", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value(&arguments[0], span)?;
                let package = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                self.packages
                    .borrow_mut()
                    .unexport_symbols(&package, &symbols);
                Ok(Value::boolean(true))
            }
            "IMPORT" | "SHADOWING-IMPORT" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity(name, "one or two", arguments.len()));
                }
                let imports = self.symbol_import_references_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                {
                    let state = self.packages.borrow();
                    for (source_package, source_name) in &imports {
                        if !state.symbol_exists(source_package, source_name) {
                            return Err(self.package_error(
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
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("shadow", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let mut state = self.packages.borrow_mut();
                for symbol in symbols {
                    state.shadow_symbol(&target, &symbol);
                }
                Ok(Value::boolean(true))
            }
            "UNINTERN" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("unintern", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
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
            _ => unreachable!("package primitive group was misclassified"),
        }
    }
}
