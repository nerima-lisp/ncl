use super::{
    Form, FormKind, HashMap, HashSet, Runtime, RuntimeError, Span, Value, atom_name,
    normalize_name, package, unqualified_name,
};

struct DefpackageSpec {
    name: String,
    nicknames: Vec<String>,
    use_packages: Vec<String>,
    exports: HashSet<String>,
    operations: Vec<DefpackageOperation>,
    documentation: Option<String>,
    local_nicknames: HashMap<String, String>,
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

impl Runtime {
    pub(super) fn special_defpackage(&self, items: &[Form]) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "defpackage",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let spec = Self::parse_defpackage(&items[1..])?;
        self.validate_defpackage(&spec, items[1].span)?;
        self.apply_defpackage(spec, items[1].span)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "defpackage option grammar is kept in one exhaustive parser"
    )]
    fn parse_defpackage(items: &[Form]) -> Result<DefpackageSpec, RuntimeError> {
        let name = Self::package_name_from_form(&items[0])?;
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

        for option in items.iter().skip(1) {
            let FormKind::List(option_items) = &option.kind else {
                return Err(Self::invalid(
                    "defpackage option must be a list",
                    option.span,
                ));
            };
            let Some(option_name) = option_items.first().and_then(atom_name) else {
                return Err(Self::invalid("defpackage option needs a name", option.span));
            };
            let normalized_option = normalize_name(option_name);
            match normalized_option.trim_start_matches(':') {
                "NICKNAMES" => {
                    if saw_nicknames {
                        return Err(Self::invalid(
                            "defpackage has duplicate :nicknames options",
                            option.span,
                        ));
                    }
                    saw_nicknames = true;
                    for package_form in option_items.iter().skip(1) {
                        nicknames.push(Self::package_name_from_form(package_form)?);
                    }
                }
                "USE" => {
                    if saw_use {
                        return Err(Self::invalid(
                            "defpackage has duplicate :use options",
                            option.span,
                        ));
                    }
                    saw_use = true;
                    use_packages.clear();
                    for package_form in option_items.iter().skip(1) {
                        use_packages.push(Self::package_name_from_form(package_form)?);
                    }
                }
                "DOCUMENTATION" => {
                    if saw_documentation || option_items.len() != 2 {
                        return Err(Self::invalid(
                            "defpackage :documentation needs one string",
                            option.span,
                        ));
                    }
                    saw_documentation = true;
                    let FormKind::String(value) = &option_items[1].kind else {
                        return Err(Self::invalid(
                            "defpackage :documentation needs a string",
                            option_items[1].span,
                        ));
                    };
                    documentation = Some(value.clone());
                }
                "SIZE" => {
                    if saw_size || option_items.len() != 2 {
                        return Err(Self::invalid(
                            "defpackage :size needs one non-negative integer",
                            option.span,
                        ));
                    }
                    saw_size = true;
                    let FormKind::Atom(value) = &option_items[1].kind else {
                        return Err(Self::invalid(
                            "defpackage :size needs a non-negative integer",
                            option_items[1].span,
                        ));
                    };
                    if value.parse::<i64>().map_or(true, |size| size < 0) {
                        return Err(Self::invalid(
                            "defpackage :size needs a non-negative integer",
                            option_items[1].span,
                        ));
                    }
                }
                "LOCAL-NICKNAMES" => {
                    for nickname_option in option_items.iter().skip(1) {
                        let FormKind::List(mapping) = &nickname_option.kind else {
                            return Err(Self::invalid(
                                "defpackage local nickname needs a two-element list",
                                nickname_option.span,
                            ));
                        };
                        if mapping.len() != 2 {
                            return Err(Self::invalid(
                                "defpackage local nickname needs a two-element list",
                                nickname_option.span,
                            ));
                        }
                        let nickname = Self::package_name_from_form(&mapping[0])?;
                        let target = Self::package_name_from_form(&mapping[1])?;
                        if local_nicknames.insert(nickname, target).is_some() {
                            return Err(Self::invalid(
                                "defpackage has duplicate local package nickname",
                                nickname_option.span,
                            ));
                        }
                    }
                }
                "EXPORT" => {
                    for symbol_form in option_items.iter().skip(1) {
                        exports.insert(Self::symbol_name_from_form(symbol_form)?);
                    }
                }
                "SHADOW" => {
                    for symbol_form in option_items.iter().skip(1) {
                        operations.push(DefpackageOperation::Shadow(Self::symbol_name_from_form(
                            symbol_form,
                        )?));
                    }
                }
                "INTERN" => {
                    for symbol_form in option_items.iter().skip(1) {
                        operations.push(DefpackageOperation::Intern(Self::symbol_name_from_form(
                            symbol_form,
                        )?));
                    }
                }
                "IMPORT-FROM" | "SHADOWING-IMPORT-FROM" => {
                    if option_items.len() < 2 {
                        return Err(Self::invalid(
                            "defpackage import option needs a package name",
                            option.span,
                        ));
                    }
                    let source_package = Self::package_name_from_form(&option_items[1])?;
                    let shadowing =
                        normalized_option.trim_start_matches(':') == "SHADOWING-IMPORT-FROM";
                    for symbol_form in option_items.iter().skip(2) {
                        operations.push(DefpackageOperation::Import {
                            source_package: source_package.clone(),
                            source_name: Self::symbol_name_from_form(symbol_form)?,
                            shadowing,
                        });
                    }
                }
                _ => {
                    return Err(Self::invalid(
                        "unsupported defpackage option",
                        option_items[0].span,
                    ));
                }
            }
        }
        Ok(DefpackageSpec {
            name,
            nicknames,
            use_packages,
            exports,
            operations,
            documentation,
            local_nicknames,
        })
    }

    fn validate_defpackage(&self, spec: &DefpackageSpec, span: Span) -> Result<(), RuntimeError> {
        let packages = self.packages.borrow();
        if let Some(missing) = spec
            .use_packages
            .iter()
            .find(|name| !packages.package_exists(name))
        {
            return Err(Self::package_error(
                &format!("unknown package {missing}"),
                span,
            ));
        }
        for operation in &spec.operations {
            let DefpackageOperation::Import {
                source_package,
                source_name,
                ..
            } = operation
            else {
                continue;
            };
            if !packages.package_exists(source_package) {
                return Err(Self::package_error(
                    &format!("unknown package {source_package}"),
                    span,
                ));
            }
            if !packages.symbol_exists(source_package, source_name) {
                return Err(Self::package_error(
                    &format!("unknown symbol {source_package}::{source_name}"),
                    span,
                ));
            }
        }
        Ok(())
    }

    fn apply_defpackage(&self, spec: DefpackageSpec, span: Span) -> Result<Value, RuntimeError> {
        let mut packages = self.packages.borrow_mut();
        if let Err(message) = packages.define_package(
            &spec.name,
            spec.nicknames,
            spec.use_packages,
            spec.exports,
            spec.documentation,
            spec.local_nicknames,
        ) {
            return Err(Self::package_error(&message, span));
        }
        for operation in spec.operations {
            match operation {
                DefpackageOperation::Shadow(symbol) => packages.shadow_symbol(&spec.name, &symbol),
                DefpackageOperation::Intern(symbol) => {
                    let _ = packages.intern_symbol(&spec.name, &symbol);
                }
                DefpackageOperation::Import {
                    source_package,
                    source_name,
                    shadowing,
                } => packages.import_symbol(&source_package, &source_name, &spec.name, shadowing),
            }
        }
        let canonical_name = packages.canonical_package_name(&spec.name);
        Ok(Value::package(&canonical_name))
    }

    pub(super) fn special_in_package(&self, items: &[Form]) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(Self::arity(
                "in-package",
                "one",
                items.len().saturating_sub(1),
            ));
        }
        let name = Self::package_name_from_form(&items[1])?;
        let mut packages = self.packages.borrow_mut();
        if !packages.package_exists(&name) {
            return Err(Self::package_error(
                &format!("unknown package {name}"),
                items[1].span,
            ));
        }
        let canonical_name = packages.canonical_package_name(&name);
        packages.set_current(&canonical_name);
        Ok(Value::package(&canonical_name))
    }

    pub(super) fn package_name_from_form(form: &Form) -> Result<String, RuntimeError> {
        let raw = match &form.kind {
            FormKind::Atom(value) | FormKind::String(value) => value.as_str(),
            _ => {
                return Err(Self::invalid(
                    "package name must be a symbol or string",
                    form.span,
                ));
            }
        };
        if !raw.starts_with(':') && package::split_symbol(raw).is_some() {
            return Err(Self::package_error(
                "package name cannot be qualified",
                form.span,
            ));
        }
        let name = package::normalize_package_name(raw);
        if name.is_empty() || name.contains(':') {
            return Err(Self::package_error("invalid package name", form.span));
        }
        Ok(name)
    }

    pub(super) fn symbol_name_from_form(form: &Form) -> Result<String, RuntimeError> {
        let raw = match &form.kind {
            FormKind::Atom(value) | FormKind::String(value) => value.as_str(),
            _ => {
                return Err(Self::invalid(
                    "symbol name must be a symbol or string",
                    form.span,
                ));
            }
        };
        let name = raw.strip_prefix(':').unwrap_or(raw);
        if name.is_empty() || name.contains(':') {
            return Err(Self::package_error(
                "symbol name cannot be qualified",
                form.span,
            ));
        }
        Ok(normalize_name(name))
    }

    pub(super) fn package_designator_name(
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
            return Err(Self::package_error(
                "package name cannot be qualified",
                span,
            ));
        }
        let name = package::normalize_package_name(raw);
        if name.is_empty() || name.contains(':') {
            return Err(Self::package_error("invalid package name", span));
        }
        Ok(name)
    }

    pub(super) fn package_name_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<String, RuntimeError> {
        let name = Self::package_designator_name(value, span)?;
        let packages = self.packages.borrow();
        if !packages.package_exists(&name) {
            return Err(Self::package_error(
                &format!("unknown package {name}"),
                span,
            ));
        }
        Ok(packages.canonical_package_name(&name))
    }

    pub(super) fn symbol_name_from_value(
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
            return Err(Self::package_error("symbol name cannot be qualified", span));
        }
        Ok(package::normalize_symbol_name(name))
    }

    pub(super) fn name_designator_from_value(
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
            return Err(Self::invalid("symbol name cannot be empty", span));
        }
        Ok(unqualified_name(name))
    }

    pub(super) fn slot_name_from_value(value: &Value, span: Span) -> Result<String, RuntimeError> {
        Self::name_designator_from_value(value, span)
    }

    pub(super) fn package_names_from_value(
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

    pub(super) fn symbol_names_from_value(
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

    pub(super) fn symbol_import_references_from_value(
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

#[cfg(test)]
mod tests {
    use super::*;

    const SPAN: Span = Span::new(0, 1);

    fn atom(name: &str) -> Form {
        Form::atom(name, SPAN)
    }

    fn string(value: &str) -> Form {
        Form::new(FormKind::String(value.to_string()), SPAN)
    }

    fn valid<T, E>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|_| panic!("expected a valid package or symbol value"))
    }

    #[test]
    fn form_name_helpers_accept_strings_and_keywords() {
        let package_cases = [("foo", "FOO"), (":bar", "BAR"), ("Baz", "BAZ")];
        for (input, expected) in package_cases {
            assert_eq!(
                valid(Runtime::package_name_from_form(&atom(input))),
                expected
            );
        }
        assert_eq!(
            valid(Runtime::package_name_from_form(&string("tools"))),
            "TOOLS"
        );

        let symbol_cases = [("foo", "FOO"), (":bar", "BAR"), ("Baz", "BAZ")];
        for (input, expected) in symbol_cases {
            assert_eq!(
                valid(Runtime::symbol_name_from_form(&atom(input))),
                expected
            );
        }
        assert_eq!(
            valid(Runtime::symbol_name_from_form(&string("tools"))),
            "TOOLS"
        );
    }

    #[test]
    fn form_name_helpers_reject_invalid_designators() {
        let invalid_forms = [
            Form::list(vec![atom("nested")], SPAN),
            atom("foo:bar"),
            atom(":"),
        ];
        for form in invalid_forms {
            assert!(Runtime::package_name_from_form(&form).is_err());
            assert!(Runtime::symbol_name_from_form(&form).is_err());
        }
    }

    #[test]
    fn value_name_helpers_cover_designators_and_errors() {
        let span = SPAN;
        let package_cases = [
            (Value::Package("user".into()), "USER"),
            (Value::String("common-lisp".into()), "COMMON-LISP"),
            (Value::symbol("keyword"), "KEYWORD"),
        ];
        for (value, expected) in package_cases {
            assert_eq!(
                valid(Runtime::package_designator_name(&value, span)),
                expected
            );
        }
        assert!(Runtime::package_designator_name(&Value::Integer(1), span).is_err());
        assert!(Runtime::package_designator_name(&Value::symbol("foo:bar"), span).is_err());

        let symbol_cases = [
            (Value::String(":name".into()), "NAME"),
            (Value::symbol("name"), "NAME"),
            (Value::keyword("key"), "KEY"),
        ];
        for (value, expected) in symbol_cases {
            assert_eq!(
                valid(Runtime::symbol_name_from_value(&value, span)),
                expected
            );
            assert_eq!(
                valid(Runtime::name_designator_from_value(&value, span)),
                expected
            );
        }
        assert!(Runtime::symbol_name_from_value(&Value::Integer(1), span).is_err());
        assert!(Runtime::name_designator_from_value(&Value::String(":".into()), span).is_err());
    }

    #[test]
    fn package_and_symbol_lists_are_table_driven() {
        let runtime = Runtime::new();
        let packages = Value::list(vec![
            Value::String("ncl-user".into()),
            Value::symbol("keyword"),
        ]);
        assert_eq!(
            valid(runtime.package_names_from_value(&packages, SPAN)),
            ["NCL-USER", "KEYWORD"]
        );

        let symbols = Value::list(vec![Value::symbol("one"), Value::keyword("two")]);
        assert_eq!(
            valid(Runtime::symbol_names_from_value(&symbols, SPAN)),
            ["ONE", "TWO"]
        );

        let invalid = Value::Integer(1);
        assert!(runtime.package_names_from_value(&invalid, SPAN).is_err());
        assert!(Runtime::symbol_names_from_value(&invalid, SPAN).is_err());
    }

    #[test]
    fn import_references_resolve_keyword_qualified_and_current_symbols() {
        let runtime = Runtime::new();
        let references = Value::list(vec![
            Value::keyword("key"),
            Value::symbol("common-lisp:car"),
            Value::symbol("local"),
        ]);
        assert_eq!(
            valid(runtime.symbol_import_references_from_value(&references, SPAN)),
            [
                ("KEYWORD".into(), "KEY".into()),
                ("COMMON-LISP".into(), "CAR".into()),
                ("NCL-USER".into(), "LOCAL".into())
            ]
        );
        assert!(
            runtime
                .symbol_import_references_from_value(&Value::Integer(1), SPAN)
                .is_err()
        );
        assert!(
            runtime
                .symbol_import_references_from_value(
                    &Value::list(vec![Value::UninternedSymbol("x".into())]),
                    SPAN
                )
                .is_err()
        );
    }

    #[test]
    fn defpackage_parser_accepts_all_options() {
        let options = vec![
            atom("TOOLS"),
            Form::list(vec![atom(":nicknames"), string("T")], SPAN),
            Form::list(vec![atom(":use"), atom("COMMON-LISP")], SPAN),
            Form::list(vec![atom(":documentation"), string("tool package")], SPAN),
            Form::list(vec![atom(":size"), atom("16")], SPAN),
            Form::list(
                vec![
                    atom(":local-nicknames"),
                    Form::list(vec![atom("CL"), atom("COMMON-LISP")], SPAN),
                ],
                SPAN,
            ),
            Form::list(vec![atom(":export"), atom("run")], SPAN),
            Form::list(vec![atom(":shadow"), atom("print")], SPAN),
            Form::list(vec![atom(":intern"), atom("state")], SPAN),
            Form::list(
                vec![atom(":import-from"), atom("COMMON-LISP"), atom("car")],
                SPAN,
            ),
            Form::list(
                vec![
                    atom(":shadowing-import-from"),
                    atom("COMMON-LISP"),
                    atom("cdr"),
                ],
                SPAN,
            ),
        ];
        let spec = valid(Runtime::parse_defpackage(&options));

        assert_eq!(spec.name, "TOOLS");
        assert_eq!(spec.nicknames, ["T"]);
        assert_eq!(spec.use_packages, ["COMMON-LISP"]);
        assert_eq!(spec.documentation.as_deref(), Some("tool package"));
        assert!(spec.exports.contains("RUN"));
        assert_eq!(
            spec.local_nicknames.get("COMMON-LISP"),
            Some(&"COMMON-LISP".to_string())
        );
        assert!(
            matches!(spec.operations[0], DefpackageOperation::Shadow(ref name) if name == "PRINT")
        );
        assert!(
            matches!(spec.operations[1], DefpackageOperation::Intern(ref name) if name == "STATE")
        );
        assert!(matches!(
            spec.operations[2],
            DefpackageOperation::Import {
                shadowing: false,
                ..
            }
        ));
        assert!(matches!(
            spec.operations[3],
            DefpackageOperation::Import {
                shadowing: true,
                ..
            }
        ));
    }

    #[test]
    fn defpackage_parser_rejects_malformed_options() {
        let cases = [
            vec![atom("TOOLS"), atom("not-a-list")],
            vec![atom("TOOLS"), Form::list(vec![], SPAN)],
            vec![
                atom("TOOLS"),
                Form::list(vec![atom(":size"), atom("-1")], SPAN),
            ],
            vec![
                atom("TOOLS"),
                Form::list(vec![atom(":local-nicknames"), atom("CL")], SPAN),
            ],
            vec![atom("TOOLS"), Form::list(vec![atom(":import-from")], SPAN)],
            vec![atom("TOOLS"), Form::list(vec![atom(":unknown")], SPAN)],
            vec![Form::new(FormKind::String(String::new()), SPAN)],
        ];

        for items in cases {
            assert!(Runtime::parse_defpackage(&items).is_err());
        }
    }
}
