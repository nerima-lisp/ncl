impl Runtime {
    fn special_defpackage(&self, items: &[Form]) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("defpackage", "at least one", items.len().saturating_sub(1)));
        }
        enum DefpackageOperation {
            Shadow(String),
            Intern(String),
            Import {
                source_package: String,
                source_name: String,
                shadowing: bool,
            },
        }

        let name = self.package_name_from_form(&items[1])?;
        let mut nicknames = Vec::new();
        let mut use_packages = vec![package::COMMON_LISP_PACKAGE.to_string()];
        let mut exports = HashSet::new();
        let mut operations = Vec::new();
        let mut saw_nicknames = false;
        let mut saw_use = false;
        let mut documentation = None;
        let mut saw_documentation = false;
        let mut saw_size = false;
        let mut local_nicknames = HashMap::new();

        for option in items.iter().skip(2) {
            let FormKind::List(option_items) = &option.kind else {
                return Err(self.invalid("defpackage option must be a list", option.span));
            };
            let Some(option_name) = option_items.first().and_then(atom_name) else {
                return Err(self.invalid("defpackage option needs a name", option.span));
            };
            let normalized_option = normalize_name(option_name);
            match normalized_option.trim_start_matches(':') {
                "NICKNAMES" => {
                    if saw_nicknames {
                        return Err(self
                            .invalid("defpackage has duplicate :nicknames options", option.span));
                    }
                    saw_nicknames = true;
                    for package_form in option_items.iter().skip(1) {
                        nicknames.push(self.package_name_from_form(package_form)?);
                    }
                }
                "USE" => {
                    if saw_use {
                        return Err(
                            self.invalid("defpackage has duplicate :use options", option.span)
                        );
                    }
                    saw_use = true;
                    use_packages.clear();
                    for package_form in option_items.iter().skip(1) {
                        use_packages.push(self.package_name_from_form(package_form)?);
                    }
                }
                "DOCUMENTATION" => {
                    if saw_documentation || option_items.len() != 2 {
                        return Err(
                            self.invalid("defpackage :documentation needs one string", option.span)
                        );
                    }
                    saw_documentation = true;
                    let FormKind::String(value) = &option_items[1].kind else {
                        return Err(self.invalid(
                            "defpackage :documentation needs a string",
                            option_items[1].span,
                        ));
                    };
                    documentation = Some(value.clone());
                }
                "SIZE" => {
                    if saw_size || option_items.len() != 2 {
                        return Err(self.invalid(
                            "defpackage :size needs one non-negative integer",
                            option.span,
                        ));
                    }
                    saw_size = true;
                    let FormKind::Atom(value) = &option_items[1].kind else {
                        return Err(self.invalid(
                            "defpackage :size needs a non-negative integer",
                            option_items[1].span,
                        ));
                    };
                    if value.parse::<i64>().map_or(true, |size| size < 0) {
                        return Err(self.invalid(
                            "defpackage :size needs a non-negative integer",
                            option_items[1].span,
                        ));
                    }
                }
                "LOCAL-NICKNAMES" => {
                    for nickname_option in option_items.iter().skip(1) {
                        let FormKind::List(mapping) = &nickname_option.kind else {
                            return Err(self.invalid(
                                "defpackage local nickname needs a two-element list",
                                nickname_option.span,
                            ));
                        };
                        if mapping.len() != 2 {
                            return Err(self.invalid(
                                "defpackage local nickname needs a two-element list",
                                nickname_option.span,
                            ));
                        }
                        let nickname = self.package_name_from_form(&mapping[0])?;
                        let target = self.package_name_from_form(&mapping[1])?;
                        if local_nicknames.insert(nickname, target).is_some() {
                            return Err(self.invalid(
                                "defpackage has duplicate local package nickname",
                                nickname_option.span,
                            ));
                        }
                    }
                }
                "EXPORT" => {
                    for symbol_form in option_items.iter().skip(1) {
                        exports.insert(self.symbol_name_from_form(symbol_form)?);
                    }
                }
                "SHADOW" => {
                    for symbol_form in option_items.iter().skip(1) {
                        operations.push(DefpackageOperation::Shadow(
                            self.symbol_name_from_form(symbol_form)?,
                        ));
                    }
                }
                "INTERN" => {
                    for symbol_form in option_items.iter().skip(1) {
                        operations.push(DefpackageOperation::Intern(
                            self.symbol_name_from_form(symbol_form)?,
                        ));
                    }
                }
                "IMPORT-FROM" | "SHADOWING-IMPORT-FROM" => {
                    if option_items.len() < 2 {
                        return Err(self.invalid(
                            "defpackage import option needs a package name",
                            option.span,
                        ));
                    }
                    let source_package = self.package_name_from_form(&option_items[1])?;
                    let shadowing =
                        normalized_option.trim_start_matches(':') == "SHADOWING-IMPORT-FROM";
                    for symbol_form in option_items.iter().skip(2) {
                        operations.push(DefpackageOperation::Import {
                            source_package: source_package.clone(),
                            source_name: self.symbol_name_from_form(symbol_form)?,
                            shadowing,
                        });
                    }
                }
                _ => {
                    return Err(self.invalid("unsupported defpackage option", option_items[0].span));
                }
            }
        }

        {
            let packages = self.packages.borrow();
            if use_packages
                .iter()
                .any(|package_name| !packages.package_exists(package_name))
            {
                let missing = use_packages
                    .iter()
                    .find(|package_name| !packages.package_exists(package_name))
                    .expect("missing package must exist");
                return Err(
                    self.package_error(&format!("unknown package {missing}"), items[1].span)
                );
            }
            for operation in &operations {
                let DefpackageOperation::Import {
                    source_package,
                    source_name,
                    ..
                } = operation
                else {
                    continue;
                };
                if !packages.package_exists(source_package) {
                    return Err(self.package_error(
                        &format!("unknown package {source_package}"),
                        items[1].span,
                    ));
                }
                if !packages.symbol_exists(source_package, source_name) {
                    return Err(self.package_error(
                        &format!("unknown symbol {source_package}::{source_name}"),
                        items[1].span,
                    ));
                }
            }
        }

        let mut packages = self.packages.borrow_mut();
        if let Err(message) = packages.define_package(
            name.clone(),
            nicknames,
            use_packages,
            exports,
            documentation,
            local_nicknames,
        ) {
            return Err(self.package_error(&message, items[1].span));
        }
        for operation in operations {
            match operation {
                DefpackageOperation::Shadow(symbol) => packages.shadow_symbol(&name, &symbol),
                DefpackageOperation::Intern(symbol) => {
                    let _ = packages.intern_symbol(&name, &symbol);
                }
                DefpackageOperation::Import {
                    source_package,
                    source_name,
                    shadowing,
                } => packages.import_symbol(&source_package, &source_name, &name, shadowing),
            }
        }
        let canonical_name = packages.canonical_package_name(&name);
        Ok(Value::package(&canonical_name))
    }

    fn special_in_package(&self, items: &[Form]) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("in-package", "one", items.len().saturating_sub(1)));
        }
        let name = self.package_name_from_form(&items[1])?;
        let mut packages = self.packages.borrow_mut();
        if !packages.package_exists(&name) {
            return Err(self.package_error(&format!("unknown package {name}"), items[1].span));
        }
        let canonical_name = packages.canonical_package_name(&name);
        packages.set_current(canonical_name.clone());
        Ok(Value::package(&canonical_name))
    }

    fn package_name_from_form(&self, form: &Form) -> Result<String, RuntimeError> {
        let raw = match &form.kind {
            FormKind::Atom(value) | FormKind::String(value) => value.as_str(),
            _ => {
                return Err(self.invalid("package name must be a symbol or string", form.span));
            }
        };
        if !raw.starts_with(':') && package::split_symbol(raw).is_some() {
            return Err(self.package_error("package name cannot be qualified", form.span));
        }
        let name = package::normalize_package_name(raw);
        if name.is_empty() || name.contains(':') {
            return Err(self.package_error("invalid package name", form.span));
        }
        Ok(name)
    }

    fn symbol_name_from_form(&self, form: &Form) -> Result<String, RuntimeError> {
        let raw = match &form.kind {
            FormKind::Atom(value) | FormKind::String(value) => value.as_str(),
            _ => return Err(self.invalid("symbol name must be a symbol or string", form.span)),
        };
        let name = raw.strip_prefix(':').unwrap_or(raw);
        if name.is_empty() || name.contains(':') {
            return Err(self.package_error("symbol name cannot be qualified", form.span));
        }
        Ok(normalize_name(name))
    }

    fn package_designator_name(&self, value: &Value, span: Span) -> Result<String, RuntimeError> {
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

    fn package_keyword_name(&self, value: &Value, span: Span) -> Result<String, RuntimeError> {
        let raw = match value {
            Value::Keyword(name) | Value::KeywordExact(name) => name.as_ref(),
            _ => {
                return Err(self.invalid("make-package options must use keyword names", span));
            }
        };
        Ok(normalize_name(raw).trim_start_matches(':').to_string())
    }

    fn make_package_from_arguments(
        &self,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() || !arguments[1..].len().is_multiple_of(2) {
            return Err(self.invalid("make-package requires a name and keyword/value pairs", span));
        }
        let name = self.package_designator_name(&arguments[0], span)?;
        let mut nicknames = Vec::new();
        let mut use_packages = Vec::new();
        let mut supplied = HashSet::new();
        for pair in arguments[1..].chunks_exact(2) {
            let keyword = self.package_keyword_name(&pair[0], span)?;
            if !supplied.insert(keyword.clone()) {
                return Err(
                    self.package_error(&format!("duplicate make-package keyword :{keyword}"), span)
                );
            }
            match keyword.as_str() {
                "NICKNAMES" => {
                    let values = pair[1].list_items().ok_or_else(|| {
                        self.invalid("package nicknames must be a proper list", span)
                    })?;
                    nicknames = values
                        .iter()
                        .map(|value| self.name_designator_from_value(value, span))
                        .collect::<Result<Vec<_>, _>>()?;
                }
                "USE" => {
                    use_packages = self.package_names_from_value(&pair[1], span)?;
                }
                _ => {
                    return Err(self
                        .package_error(&format!("unknown make-package keyword :{keyword}"), span));
                }
            }
        }
        let name = self
            .packages
            .borrow_mut()
            .make_package(name, nicknames, use_packages, None)
            .map_err(|message| self.package_error(&message, span))?;
        Ok(Value::package(name))
    }

    fn package_nicknames_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<String>, RuntimeError> {
        let values = value
            .list_items()
            .ok_or_else(|| self.invalid("package nicknames must be a proper list", span))?;
        values
            .iter()
            .map(|value| self.name_designator_from_value(value, span))
            .collect()
    }

    fn package_name_from_value(&self, value: &Value, span: Span) -> Result<String, RuntimeError> {
        let name = self.package_designator_name(value, span)?;
        let packages = self.packages.borrow();
        if !packages.package_exists(&name) {
            return Err(self.package_error(&format!("unknown package {name}"), span));
        }
        Ok(packages.canonical_package_name(&name))
    }

    fn symbol_name_from_value(&self, value: &Value, span: Span) -> Result<String, RuntimeError> {
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

    fn name_designator_from_value(
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

    fn slot_name_from_value(&self, value: &Value, span: Span) -> Result<String, RuntimeError> {
        self.name_designator_from_value(value, span)
    }

    fn package_names_from_value(
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

    fn symbol_names_from_value(
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

    fn symbol_import_references_from_value(
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

    fn package_symbol_value(&self, package_name: &str, symbol_name: &str) -> Value {
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

    fn symbol_status_value(status: package::SymbolStatus) -> Value {
        match status {
            package::SymbolStatus::Internal => Value::keyword("INTERNAL"),
            package::SymbolStatus::External => Value::keyword("EXTERNAL"),
        }
    }


}
