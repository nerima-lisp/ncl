impl Runtime {
    fn setf_index(&self, value: Value, span: Span) -> Result<usize, RuntimeError> {
        match value {
            Value::Integer(index) if index >= 0 => {
                usize::try_from(index).map_err(|_| self.invalid("SETF index is too large", span))
            }
            Value::Integer(_) => Err(self.invalid("SETF index must be non-negative", span)),
            other => Err(RuntimeError::Type {
                expected: "INTEGER".to_string(),
                actual: other.type_name().to_string(),
                span: Some(span),
            }),
        }
    }

    fn special_defvar(
        &self,
        items: &[Form],
        environment: &Environment,
        force: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if force { "defparameter" } else { "defvar" };
        if !(2..=4).contains(&items.len()) {
            return Err(self.arity(operator, "one to three", items.len().saturating_sub(1)));
        }
        let context = if force {
            "defparameter name must be a symbol"
        } else {
            "defvar name must be a symbol"
        };
        let (name, escaped) = self.variable_name_info(&items[1], context)?;
        let documentation = match items.get(3) {
            Some(Form {
                kind: FormKind::String(documentation),
                ..
            }) => Some(documentation.clone()),
            Some(form) => {
                return Err(self.invalid("defvar documentation must be a string", form.span));
            }
            None => None,
        };
        if force
            && if escaped {
                self.is_constant_exact_in(&name)
            } else {
                self.is_constant_in(&name)
            }
        {
            return Err(self.constant_modification_error(&name, items[1].span));
        }
        if !force {
            let existing = if escaped {
                self.lookup_special_exact(&name)
            } else {
                self.lookup_special(&name)
            };
            if let Some(value) = existing {
                if let Some(documentation) = documentation {
                    if escaped {
                        environment.define_variable_documentation_exact(&name, documentation);
                    } else {
                        environment.define_variable_documentation(&name, documentation);
                    }
                }
                return Ok(value);
            }
        };
        let value = items
            .get(2)
            .map_or(Ok(Value::Nil), |form| self.eval_in(form, environment))?;
        let value = if escaped {
            self.define_special_value_exact(&name, value, force)
        } else {
            self.define_special_value(&name, value, force)
        };
        if let Some(documentation) = documentation {
            if escaped {
                environment.define_variable_documentation_exact(&name, documentation);
            } else {
                environment.define_variable_documentation(&name, documentation);
            }
        }
        Ok(value)
    }

    fn special_defconstant(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(self.arity("defconstant", "two or three", items.len().saturating_sub(1)));
        }
        let (name, escaped) =
            self.variable_name_info(&items[1], "defconstant name must be a symbol")?;
        if if escaped {
            self.is_constant_exact_in(&name)
        } else {
            self.is_constant_in(&name)
        } {
            return Err(self.constant_modification_error(&name, items[1].span));
        }
        let value = self.eval_in(&items[2], environment)?;
        Ok(if escaped {
            self.define_constant_value_exact(&name, value)
        } else {
            self.define_constant_value(&name, value)
        })
    }

    fn special_defstruct(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("defstruct", "at least one", items.len().saturating_sub(1)));
        }
        let (name_form, option_forms, slot_forms) = match &items[1].kind {
            FormKind::Atom(_) => (&items[1], &items[2..2], &items[2..]),
            FormKind::List(name_and_options) if !name_and_options.is_empty() => {
                (&name_and_options[0], &name_and_options[1..], &items[2..])
            }
            _ => {
                return Err(self.invalid(
                    "defstruct name must be a symbol or a name-and-options list",
                    items[1].span,
                ));
            }
        };
        let (raw_name, _) =
            self.variable_name_info(name_form, "defstruct name must be a symbol")?;
        let structure_name = unqualified_name(&raw_name);
        let mut conc_name = format!("{structure_name}-");
        let mut predicate_name = Some(format!("{structure_name}-P"));
        let mut copier_name = Some(format!("COPY-{structure_name}"));
        let mut constructor_options: Vec<(Option<String>, Option<OrdinaryLambdaList>)> = Vec::new();
        let mut seen_options = HashSet::new();
        let mut included_structure: Option<(StructureDefinition, Vec<Form>)> = None;
        for option_form in option_forms {
            let FormKind::List(option_items) = &option_form.kind else {
                return Err(self.invalid("defstruct option must be a list", option_form.span));
            };
            let Some(option_name) = option_items.first().and_then(atom_name) else {
                return Err(self.invalid("defstruct option needs a name", option_form.span));
            };
            let normalized_option = normalize_name(option_name);
            let option_name = normalized_option.trim_start_matches(':');
            if option_name != "CONSTRUCTOR" && !seen_options.insert(option_name.to_string()) {
                return Err(self.invalid("defstruct cannot repeat an option", option_form.span));
            }
            match option_name {
                "CONC-NAME" => {
                    conc_name = self
                        .defstruct_name_option(
                            option_form,
                            option_items,
                            format!("{structure_name}-"),
                            "defstruct :conc-name must name a symbol or NIL",
                        )?
                        .unwrap_or_default();
                }
                "PREDICATE" => {
                    predicate_name = self.defstruct_name_option(
                        option_form,
                        option_items,
                        format!("{structure_name}-P"),
                        "defstruct :predicate must name a symbol or NIL",
                    )?;
                }
                "COPIER" => {
                    copier_name = self.defstruct_name_option(
                        option_form,
                        option_items,
                        format!("COPY-{structure_name}"),
                        "defstruct :copier must name a symbol or NIL",
                    )?;
                }
                "NAMED" => {
                    if option_items.len() != 1 {
                        return Err(self.invalid(
                            "defstruct :named does not accept arguments",
                            option_form.span,
                        ));
                    }
                }
                "INCLUDE" => {
                    if option_items.len() < 2 {
                        return Err(self.invalid(
                            "defstruct :include needs a structure name",
                            option_form.span,
                        ));
                    }
                    let (raw_parent_name, _) = self.variable_name_info(
                        &option_items[1],
                        "defstruct :include structure name must be a symbol",
                    )?;
                    let parent_name = unqualified_name(&raw_parent_name);
                    let Some(parent) = environment.lookup_structure(&parent_name) else {
                        return Err(self.invalid(
                            "defstruct :include requires a previously defined structure",
                            option_form.span,
                        ));
                    };
                    included_structure = Some((parent, option_items[2..].to_vec()));
                }
                "CONSTRUCTOR" => {
                    let constructor = self.defstruct_constructor_option(
                        option_form,
                        option_items,
                        format!("MAKE-{structure_name}"),
                    )?;
                    if (constructor.0.is_none() && !constructor_options.is_empty())
                        || constructor_options.iter().any(|(name, _)| name.is_none())
                    {
                        return Err(self.invalid(
                            "defstruct :constructor NIL cannot be combined with another constructor",
                            option_form.span,
                        ));
                    }
                    constructor_options.push(constructor);
                }
                _ => {
                    return Err(self.invalid("unsupported defstruct option", option_items[0].span));
                }
            }
        }
        let mut structure_types = vec![structure_name.clone()];
        let mut slots = Vec::new();
        let mut slot_names = HashSet::new();
        if let Some((parent, overrides)) = included_structure {
            structure_types.extend(parent.type_names.clone());
            slots = parent.slots.clone();
            for slot in &slots {
                slot_names.insert(slot.name.clone());
            }
            let mut overridden_slots = HashSet::new();
            for slot_form in overrides {
                let (raw_slot_name, init_form, read_only) =
                    self.defstruct_slot_description(&slot_form, environment)?;
                let slot_name = unqualified_name(&raw_slot_name);
                let Some(slot) = slots.iter_mut().find(|slot| slot.name == slot_name) else {
                    return Err(self.invalid(
                        "defstruct :include slot must name an inherited slot",
                        slot_form.span,
                    ));
                };
                if !overridden_slots.insert(slot_name) {
                    return Err(self.invalid(
                        "defstruct :include cannot override a slot more than once",
                        slot_form.span,
                    ));
                }
                if let Some(init_form) = init_form {
                    slot.init_form = Some(init_form);
                }
                if let Some(read_only) = read_only {
                    slot.read_only = read_only;
                }
            }
        }
        for slot_form in slot_forms {
            let (raw_slot_name, init_form, read_only) =
                self.defstruct_slot_description(slot_form, environment)?;
            let slot_name = unqualified_name(&raw_slot_name);
            if !slot_names.insert(slot_name.clone()) {
                return Err(self.invalid("defstruct cannot define duplicate slots", slot_form.span));
            }
            slots.push(StructureSlot {
                name: slot_name,
                init_form,
                read_only: read_only.unwrap_or(false),
            });
        }

        environment.define_structure(
            &structure_name,
            StructureDefinition {
                slots: slots.clone(),
                type_names: structure_types.clone(),
            },
        );
        if constructor_options.is_empty() {
            constructor_options.push((Some(format!("MAKE-{structure_name}")), None));
        }
        for (constructor_name, constructor_lambda_list) in constructor_options {
            if let Some(constructor_name) = constructor_name {
                environment.define_function(
                    &constructor_name,
                    Value::Function(Rc::new(crate::Function::StructureConstructor {
                        name: structure_name.clone(),
                        slots: slots.clone(),
                        structure_types: structure_types.clone(),
                        constructor_lambda_list,
                        environment: environment.clone(),
                    })),
                );
            }
        }
        if let Some(predicate_name) = predicate_name {
            environment.define_function(
                &predicate_name,
                Value::Function(Rc::new(crate::Function::StructurePredicate {
                    name: structure_name.clone(),
                })),
            );
        }
        if let Some(copier_name) = copier_name {
            environment.define_function(
                &copier_name,
                Value::Function(Rc::new(crate::Function::StructureCopier {
                    name: structure_name.clone(),
                })),
            );
        }
        let conc_name = conc_name;
        for (slot_index, slot) in slots.iter().enumerate() {
            let accessor_name = format!("{conc_name}{}", slot.name);
            environment.define_function(
                &accessor_name,
                Value::Function(Rc::new(crate::Function::StructureAccessor {
                    structure_name: structure_name.clone(),
                    slot_name: slot.name.clone(),
                    slot_index,
                    read_only: slot.read_only,
                })),
            );
        }
        Ok(Value::symbol(structure_name))
    }

    fn special_define_condition(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.arity("define-condition", "four", items.len().saturating_sub(1)));
        }

        let condition_name =
            self.variable_name(&items[1], "define-condition name must be a symbol")?;
        let condition_name = unqualified_name(&condition_name);
        let superclass_forms =
            self.list_form_items(&items[2], "define-condition superclass list")?;
        let mut direct_superclasses = Vec::with_capacity(superclass_forms.len().max(1));
        for superclass in superclass_forms {
            let name = self.definition_name_from_form(superclass, "define-condition superclass")?;
            if !direct_superclasses.contains(&name) {
                direct_superclasses.push(name);
            }
        }
        if direct_superclasses.is_empty() {
            direct_superclasses.push("CONDITION".to_owned());
        }

        let slot_forms = self.list_form_items(&items[3], "define-condition slot list")?;
        let mut slots: Vec<ConditionSlot> = Vec::new();
        for slot_form in slot_forms {
            let (slot_name_form, options) = match &slot_form.kind {
                FormKind::Atom(_) => (slot_form, &[][..]),
                FormKind::List(slot_items) if !slot_items.is_empty() => {
                    (&slot_items[0], &slot_items[1..])
                }
                _ => {
                    return Err(self.invalid(
                        "define-condition slot must be a symbol or non-empty list",
                        slot_form.span,
                    ));
                }
            };
            let slot_name =
                self.variable_name(slot_name_form, "define-condition slot must be a symbol")?;
            let slot_name = unqualified_name(&slot_name);
            if options.len() % 2 != 0 {
                return Err(self.invalid(
                    "define-condition slot options require a value",
                    slot_form.span,
                ));
            }

            let mut initarg = None;
            let mut init_form = None;
            let mut readers = Vec::new();
            let mut writers = Vec::new();
            for option in options.chunks_exact(2) {
                let option_name =
                    self.definition_name_from_form(&option[0], "define-condition slot option")?;
                match option_name.as_str() {
                    "INITARG" => {
                        initarg = if is_nil_form(&option[1]) {
                            None
                        } else {
                            Some(self.definition_name_from_form(
                                &option[1],
                                "define-condition initarg",
                            )?)
                        };
                    }
                    "INITFORM" => init_form = Some(option[1].clone()),
                    "ACCESSOR" | "READER" => {
                        let accessor_name = self.variable_name(
                            &option[1],
                            "define-condition accessor must be a symbol",
                        )?;
                        readers.push(unqualified_name(&accessor_name));
                    }
                    "WRITER" => {
                        let writer_name = self.variable_name(
                            &option[1],
                            "define-condition writer must be a symbol",
                        )?;
                        writers.push(unqualified_name(&writer_name));
                    }
                    "TYPE" | "DOCUMENTATION" => {}
                    _ => {
                        return Err(self
                            .invalid("unsupported define-condition slot option", option[0].span));
                    }
                }
            }

            let slot = ConditionSlot {
                name: slot_name.clone(),
                initarg,
                init_form,
                readers,
                writers,
            };
            if let Some(existing) = slots.iter_mut().find(|slot| slot.name == slot_name) {
                *existing = slot;
            } else {
                slots.push(slot);
            }
        }

        let mut report = None;
        for option in items.iter().skip(4) {
            let option_items = self.list_form_items(option, "define-condition option")?;
            if option_items.is_empty() {
                return Err(self.invalid(
                    "define-condition option must be a non-empty list",
                    option.span,
                ));
            }
            let option_name =
                self.definition_name_from_form(&option_items[0], "define-condition option name")?;
            match option_name.as_str() {
                "REPORT" => {
                    if option_items.len() != 2 {
                        return Err(
                            self.invalid("define-condition :report needs one value", option.span)
                        );
                    }
                    report = match &option_items[1].kind {
                        FormKind::String(value) => Some(value.to_string()),
                        _ => Some(self.definition_name_from_form(
                            &option_items[1],
                            "define-condition report",
                        )?),
                    };
                }
                "DOCUMENTATION" => {
                    if option_items.len() != 2
                        || !matches!(option_items[1].kind, FormKind::String(_))
                    {
                        return Err(self.invalid(
                            "define-condition :documentation needs one string",
                            option.span,
                        ));
                    }
                }
                _ => {
                    return Err(
                        self.invalid("unsupported define-condition option", option_items[0].span)
                    );
                }
            }
        }

        let mut precedence = vec![condition_name.clone()];
        for superclass in &direct_superclasses {
            let parent_definition = environment.lookup_condition(superclass);
            let parent_precedence = match superclass.as_str() {
                "CONDITION" => vec!["CONDITION".to_owned()],
                "SERIOUS-CONDITION" => {
                    vec!["SERIOUS-CONDITION".to_owned(), "CONDITION".to_owned()]
                }
                "WARNING" => vec!["WARNING".to_owned(), "CONDITION".to_owned()],
                "ERROR" => vec![
                    "ERROR".to_owned(),
                    "SERIOUS-CONDITION".to_owned(),
                    "CONDITION".to_owned(),
                ],
                "SIMPLE-CONDITION" => {
                    vec!["SIMPLE-CONDITION".to_owned(), "CONDITION".to_owned()]
                }
                "SIMPLE-ERROR" => vec![
                    "SIMPLE-ERROR".to_owned(),
                    "SIMPLE-CONDITION".to_owned(),
                    "ERROR".to_owned(),
                    "SERIOUS-CONDITION".to_owned(),
                    "CONDITION".to_owned(),
                ],
                "SIMPLE-WARNING" => vec![
                    "SIMPLE-WARNING".to_owned(),
                    "SIMPLE-CONDITION".to_owned(),
                    "WARNING".to_owned(),
                    "CONDITION".to_owned(),
                ],
                "ARITHMETIC-ERROR" => vec![
                    "ARITHMETIC-ERROR".to_owned(),
                    "ERROR".to_owned(),
                    "SERIOUS-CONDITION".to_owned(),
                    "CONDITION".to_owned(),
                ],
                "DIVISION-BY-ZERO" => vec![
                    "DIVISION-BY-ZERO".to_owned(),
                    "ARITHMETIC-ERROR".to_owned(),
                    "ERROR".to_owned(),
                    "SERIOUS-CONDITION".to_owned(),
                    "CONDITION".to_owned(),
                ],
                "TYPE-ERROR" | "PROGRAM-ERROR" | "PACKAGE-ERROR" | "READER-ERROR"
                | "COMPILER-ERROR" | "FILE-ERROR" | "UNBOUND-VARIABLE" => vec![
                    superclass.clone(),
                    "ERROR".to_owned(),
                    "SERIOUS-CONDITION".to_owned(),
                    "CONDITION".to_owned(),
                ],
                "CONTROL-ERROR" => {
                    vec!["CONTROL-ERROR".to_owned(), "CONDITION".to_owned()]
                }
                _ => parent_definition
                    .as_ref()
                    .map(|definition| definition.precedence.clone())
                    .ok_or_else(|| {
                        self.invalid("unknown define-condition superclass", items[2].span)
                    })?,
            };
            for name in parent_precedence {
                if !precedence.iter().any(|existing| existing == &name) {
                    precedence.push(name);
                }
            }
            if let Some(definition) = parent_definition {
                for inherited in &definition.slots {
                    if !slots.iter().any(|slot| slot.name == inherited.name) {
                        slots.push(inherited.clone());
                    }
                }
            }
        }
        if !precedence.iter().any(|name| name == "CONDITION") {
            precedence.push("CONDITION".to_owned());
        }

        let definition = Rc::new(ConditionDefinition {
            precedence,
            slots,
            report,
        });
        environment.define_condition(&condition_name, definition.clone());
        for slot in &definition.slots {
            for reader_name in &slot.readers {
                environment.define_function(
                    reader_name,
                    Value::condition_reader(condition_name.clone(), slot.name.clone()),
                );
            }
            for writer_name in &slot.writers {
                environment.define_function(
                    writer_name,
                    Value::condition_writer(condition_name.clone(), slot.name.clone()),
                );
            }
        }
        Ok(Value::symbol(condition_name))
    }

    fn class_precedence(
        &self,
        class_name: &str,
        direct_superclasses: &[String],
        environment: &Environment,
        span: Span,
    ) -> Result<Vec<String>, RuntimeError> {
        let mut effective_superclasses = Vec::new();
        if direct_superclasses.is_empty() {
            effective_superclasses.push("STANDARD-OBJECT".to_owned());
        } else {
            for superclass in direct_superclasses {
                let superclass = if superclass == "OBJECT" {
                    "STANDARD-OBJECT".to_owned()
                } else {
                    superclass.clone()
                };
                if !effective_superclasses.contains(&superclass) {
                    effective_superclasses.push(superclass);
                }
            }
        }

        let mut sequences = Vec::with_capacity(effective_superclasses.len() + 1);
        for superclass in &effective_superclasses {
            let precedence = if superclass == "STANDARD-OBJECT" {
                vec!["STANDARD-OBJECT".to_owned()]
            } else {
                let Some(definition) = environment.lookup_class(superclass) else {
                    return Err(self.invalid("unknown defclass superclass", span));
                };
                definition.precedence.clone()
            };
            sequences.push(precedence);
        }
        sequences.push(effective_superclasses);

        let mut precedence = vec![class_name.to_owned()];
        while sequences.iter().any(|sequence| !sequence.is_empty()) {
            let Some(candidate) = sequences
                .iter()
                .filter_map(|sequence| sequence.first())
                .find(|candidate| {
                    !sequences
                        .iter()
                        .any(|sequence| sequence.iter().skip(1).any(|name| name == *candidate))
                })
                .cloned()
            else {
                return Err(self.invalid("inconsistent class precedence order", span));
            };
            precedence.push(candidate.clone());
            for sequence in &mut sequences {
                if sequence.first() == Some(&candidate) {
                    sequence.remove(0);
                }
            }
        }
        Ok(precedence)
    }

    fn special_defclass(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.arity("defclass", "four", items.len().saturating_sub(1)));
        }

        let class_name = self.variable_name(&items[1], "defclass name must be a symbol")?;
        let class_name = unqualified_name(&class_name);
        let superclasses = self.list_form_items(&items[2], "defclass superclass list")?;
        let mut direct_superclasses = Vec::with_capacity(superclasses.len());
        for superclass in superclasses {
            let name = self.definition_name_from_form(superclass, "defclass superclass")?;
            if direct_superclasses.contains(&name) {
                return Err(self.invalid("duplicate defclass superclass", superclass.span));
            }
            direct_superclasses.push(name);
        }

        let slot_forms = self.list_form_items(&items[3], "defclass slot list")?;
        let mut slots: Vec<ClassSlot> = Vec::new();
        let mut readers = Vec::new();
        let mut writers = Vec::new();
        let mut default_initargs = Vec::new();
        let mut documentation = None;

        for slot_form in slot_forms {
            let (slot_name_form, options) = match &slot_form.kind {
                FormKind::Atom(_) => (slot_form, &[][..]),
                FormKind::List(slot_items) if !slot_items.is_empty() => {
                    (&slot_items[0], &slot_items[1..])
                }
                _ => {
                    return Err(self.invalid(
                        "defclass slot must be a symbol or non-empty list",
                        slot_form.span,
                    ));
                }
            };
            let slot_name = self.variable_name(slot_name_form, "defclass slot must be a symbol")?;
            let slot_name = unqualified_name(&slot_name);
            let mut initarg = None;
            let mut init_form = None;
            let mut class_value = None;

            if options.len() % 2 != 0 {
                return Err(self.invalid("defclass slot options require a value", slot_form.span));
            }
            for option in options.chunks_exact(2) {
                let option_name =
                    self.definition_name_from_form(&option[0], "defclass slot option")?;
                match option_name.as_str() {
                    "INITARG" => {
                        initarg = if is_nil_form(&option[1]) {
                            None
                        } else {
                            Some(self.definition_name_from_form(&option[1], "defclass initarg")?)
                        };
                    }
                    "INITFORM" => init_form = Some(option[1].clone()),
                    "ALLOCATION" => {
                        let allocation =
                            self.definition_name_from_form(&option[1], "defclass allocation")?;
                        match allocation.as_str() {
                            "CLASS" => {
                                class_value = Some(Rc::new(RefCell::new(Value::Unbound)));
                            }
                            "INSTANCE" => {
                                class_value = None;
                            }
                            _ => {
                                return Err(
                                    self.invalid("unsupported defclass allocation", option[1].span)
                                );
                            }
                        }
                    }
                    "ACCESSOR" | "READER" => {
                        let accessor_name =
                            self.variable_name(&option[1], "defclass accessor must be a symbol")?;
                        readers.push((unqualified_name(&accessor_name), slot_name.clone()));
                    }
                    "WRITER" => {
                        let writer_name =
                            self.variable_name(&option[1], "defclass writer must be a symbol")?;
                        writers.push((unqualified_name(&writer_name), slot_name.clone()));
                    }
                    "TYPE" | "DOCUMENTATION" => {}
                    _ => {
                        return Err(
                            self.invalid("unsupported defclass slot option", option[0].span)
                        );
                    }
                }
            }

            if slots.iter().any(|slot| slot.name == slot_name) {
                return Err(self.invalid("duplicate defclass slot name", slot_name_form.span));
            }

            slots.push(ClassSlot {
                name: slot_name,
                initarg,
                init_form,
                class_value,
            });
        }

        for option in items.iter().skip(4) {
            let option_items = self.list_form_items(option, "defclass option")?;
            if option_items.is_empty() {
                return Err(self.invalid("defclass option must be a non-empty list", option.span));
            }
            let option_name =
                self.definition_name_from_form(&option_items[0], "defclass option name")?;
            match option_name.as_str() {
                "DEFAULT-INITARGS" => {
                    if option_items.len() < 3 || (option_items.len() - 1) % 2 != 0 {
                        return Err(self.invalid(
                            "defclass :default-initargs requires initarg and form pairs",
                            option.span,
                        ));
                    }
                    for pair in option_items[1..].chunks_exact(2) {
                        let initarg =
                            self.definition_name_from_form(&pair[0], "defclass default initarg")?;
                        if let Some(existing) = default_initargs
                            .iter_mut()
                            .find(|(name, _)| name == &initarg)
                        {
                            existing.1 = pair[1].clone();
                        } else {
                            default_initargs.push((initarg, pair[1].clone()));
                        }
                    }
                }
                "DOCUMENTATION" => {
                    if option_items.len() != 2
                        || !matches!(option_items[1].kind, FormKind::String(_))
                    {
                        return Err(
                            self.invalid("defclass :documentation needs one string", option.span)
                        );
                    }
                    let FormKind::String(value) = &option_items[1].kind else {
                        unreachable!("defclass :documentation string was already validated");
                    };
                    documentation = Some(value.to_string());
                }
                "METACLASS" => {
                    if option_items.len() != 2 {
                        return Err(
                            self.invalid("defclass :metaclass needs one class name", option.span)
                        );
                    }
                    let metaclass =
                        self.definition_name_from_form(&option_items[1], "defclass metaclass")?;
                    if metaclass != "STANDARD-CLASS" {
                        return Err(self.invalid("unsupported defclass metaclass", option.span));
                    }
                }
                _ => {
                    return Err(self.invalid("unsupported defclass option", option_items[0].span));
                }
            }
        }

        let precedence = self.class_precedence(
            &class_name,
            &direct_superclasses,
            environment,
            items[2].span,
        )?;
        for superclass in &direct_superclasses {
            if superclass == "OBJECT" || superclass == "STANDARD-OBJECT" {
                continue;
            }
            let Some(definition) = environment.lookup_class(superclass) else {
                return Err(self.invalid("unknown defclass superclass", items[2].span));
            };
            for inherited in &definition.slots {
                if !slots.iter().any(|slot| slot.name == inherited.name) {
                    slots.push(inherited.clone());
                }
            }
            for inherited in &definition.default_initargs {
                if !default_initargs
                    .iter()
                    .any(|(name, _)| name == &inherited.0)
                {
                    default_initargs.push(inherited.clone());
                }
            }
        }

        let definition = Rc::new(ClassDefinition {
            name: class_name.clone(),
            precedence,
            slots,
            default_initargs,
            documentation: Rc::new(RefCell::new(documentation)),
        });
        environment.define_class(&class_name, definition);
        for (accessor_name, slot_name) in readers {
            environment.define_function(
                &accessor_name,
                Value::slot_reader(class_name.clone(), slot_name),
            );
        }
        for (writer_name, slot_name) in writers {
            environment.define_function(
                &writer_name,
                Value::slot_writer(class_name.clone(), slot_name),
            );
        }
        Ok(Value::symbol(class_name))
    }

    fn special_defgeneric(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("defgeneric", "three", items.len().saturating_sub(1)));
        }
        let name = self.variable_name(&items[1], "defgeneric name must be a symbol")?;
        let name = unqualified_name(&name);
        let lambda_list = self.parameters(&items[2])?;
        let mut documentation = None;
        match environment.lookup_function(&name) {
            Some(Value::Function(function)) => match function.as_ref() {
                crate::Function::Generic {
                    lambda_list: existing,
                    ..
                } => self.ensure_generic_lambda_list_congruence(
                    existing,
                    &lambda_list,
                    items[2].span,
                )?,
                _ => {
                    return Err(
                        self.invalid("defgeneric name is not a generic function", items[1].span)
                    );
                }
            },
            Some(_) => {
                return Err(
                    self.invalid("defgeneric name is not a generic function", items[1].span)
                );
            }
            None => {
                environment.define_function(&name, Value::generic(name.clone(), lambda_list));
            }
        }
        for option in items.iter().skip(3) {
            let option_items = self.list_form_items(option, "defgeneric option")?;
            let Some(option_name_form) = option_items.first() else {
                return Err(self.invalid("defgeneric option must be non-empty", option.span));
            };
            let option_name =
                self.definition_name_from_form(option_name_form, "defgeneric option name")?;
            match option_name.as_str() {
                "METHOD" => {
                    if option_items.len() < 3 {
                        return Err(self.invalid(
                            "defgeneric :method option requires a lambda list and body",
                            option.span,
                        ));
                    }
                    let mut method_items = Vec::with_capacity(option_items.len() + 1);
                    method_items.push(Form::atom("DEFMETHOD", option.span));
                    method_items.push(items[1].clone());
                    method_items.extend(option_items[1..].iter().cloned());
                    self.special_defmethod(&method_items, environment)?;
                }
                "DOCUMENTATION" => {
                    if option_items.len() != 2 {
                        return Err(
                            self.invalid("defgeneric :documentation needs one string", option.span)
                        );
                    }
                    let FormKind::String(value) = &option_items[1].kind else {
                        return Err(self.invalid(
                            "defgeneric :documentation needs a string",
                            option_items[1].span,
                        ));
                    };
                    documentation = Some(value.clone());
                }
                _ => {}
            }
        }
        if let Some(documentation) = documentation {
            environment.define_function_documentation(&name, documentation);
        }
        Ok(Value::symbol(name))
    }

    fn ensure_generic_function(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(self.arity("ensure-generic-function", "at least one", arguments.len()));
        }
        if !(arguments.len() - 1).is_multiple_of(2) {
            return Err(self.invalid(
                "ensure-generic-function keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let (raw_name, exact) = arguments[0]
            .symbol_reference()
            .ok_or_else(|| self.invalid("ensure-generic-function name must be a symbol", span))?;
        let name = if exact {
            raw_name.to_owned()
        } else {
            unqualified_name(raw_name)
        };

        let mut allow_other_keys = false;
        for pair in arguments[1..].chunks_exact(2) {
            let Some((keyword, _)) = pair[0].symbol_reference() else {
                return Err(self.invalid(
                    "ensure-generic-function keyword name must be a symbol",
                    span,
                ));
            };
            if normalize_name(keyword).trim_start_matches(':') == "ALLOW-OTHER-KEYS"
                && pair[1].is_truthy()
            {
                allow_other_keys = true;
                break;
            }
        }

        let mut lambda_list = None;
        for pair in arguments[1..].chunks_exact(2) {
            let Some((keyword, _)) = pair[0].symbol_reference() else {
                return Err(self.invalid(
                    "ensure-generic-function keyword name must be a symbol",
                    span,
                ));
            };
            let normalized = normalize_name(keyword);
            let keyword = normalized.trim_start_matches(':');
            match keyword {
                "LAMBDA-LIST" => {
                    let form = self.form_from_value(&pair[1], span)?;
                    lambda_list = Some(self.parameters(&form)?);
                }
                "ARGUMENT-PRECEDENCE-ORDER"
                | "DECLARE"
                | "DOCUMENTATION"
                | "ENVIRONMENT"
                | "GENERIC-FUNCTION-CLASS"
                | "METHOD-CLASS"
                | "METHOD-COMBINATION"
                | "ALLOW-OTHER-KEYS" => {}
                _ if allow_other_keys => {}
                _ => {
                    return Err(self.invalid("unknown ensure-generic-function keyword", span));
                }
            }
        }

        let existing = if exact {
            self.lookup_function_exact_in(raw_name, environment)
        } else {
            self.lookup_function_in(&name, environment)
        };
        match existing {
            Some(Value::Function(function)) => match function.as_ref() {
                crate::Function::Generic {
                    lambda_list: existing,
                    ..
                } => {
                    if let Some(lambda_list) = &lambda_list {
                        self.ensure_generic_lambda_list_congruence(existing, lambda_list, span)?;
                    }
                    Ok(Value::Function(function))
                }
                _ => Err(self.invalid(
                    "ensure-generic-function name is not a generic function",
                    span,
                )),
            },
            Some(_) => Err(self.invalid(
                "ensure-generic-function name is not a generic function",
                span,
            )),
            None => {
                let lambda_list = match lambda_list {
                    Some(lambda_list) => lambda_list,
                    None => self.parameters(&Form::list(
                        vec![Form::atom("&REST", span), Form::atom("ARGUMENTS", span)],
                        span,
                    ))?,
                };
                let generic = Value::generic(name.clone(), lambda_list);
                if exact {
                    environment.define_function_exact(&name, generic.clone());
                } else {
                    environment.define_function(&name, generic.clone());
                }
                Ok(generic)
            }
        }
    }

    fn find_method(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !(3..=4).contains(&arguments.len()) {
            return Err(self.arity("find-method", "three or four", arguments.len()));
        }
        let function = self.resolve_function_designator(&arguments[0], span, environment)?;
        let qualifiers = self.method_qualifiers_from_value(&arguments[1], span)?;
        let specializers = self.method_specializers_from_value(&arguments[2], span)?;
        let errorp = arguments.get(3).is_none_or(Value::is_truthy);
        let crate::Function::Generic { methods, .. } = function.as_ref() else {
            return Err(RuntimeError::Type {
                expected: "GENERIC-FUNCTION".to_owned(),
                actual: Value::Function(function).type_name().to_string(),
                span: Some(span),
            });
        };

        let methods = methods.borrow();
        let method = methods.iter().find(|method| {
            method.qualifiers == qualifiers
                && method.specializers.len() == specializers.len()
                && method
                    .specializers
                    .iter()
                    .zip(specializers.iter())
                    .all(|(left, right)| self.same_method_specializer(left, right))
        });
        match method {
            Some(method) => Ok(Value::Method(Rc::new(method.clone()))),
            None if errorp => Err(self.invalid("method not found", span)),
            None => Ok(Value::Nil),
        }
    }

    fn method_qualifiers_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<String>, RuntimeError> {
        let items = value
            .list_items()
            .ok_or_else(|| self.invalid("find-method qualifiers must be a proper list", span))?;
        items
            .iter()
            .map(|item| {
                let (name, _) = item.symbol_reference().ok_or_else(|| RuntimeError::Type {
                    expected: "SYMBOL".to_owned(),
                    actual: item.type_name().to_string(),
                    span: Some(span),
                })?;
                Ok(normalize_name(name).trim_start_matches(':').to_owned())
            })
            .collect()
    }

    fn method_specializers_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<MethodSpecializer>, RuntimeError> {
        let items = value
            .list_items()
            .ok_or_else(|| self.invalid("find-method specializers must be a proper list", span))?;
        items
            .iter()
            .map(|item| self.method_specializer_from_value(item, span))
            .collect()
    }

    fn method_specializer_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<MethodSpecializer, RuntimeError> {
        if let Value::Class(class) = value {
            return Ok(MethodSpecializer::Class(class.name.clone()));
        }
        if let Some((name, exact)) = value.symbol_reference() {
            let class = if exact {
                name.to_owned()
            } else {
                unqualified_name(name)
            };
            return Ok(MethodSpecializer::Class(class));
        }
        if let Some(items) = value.list_items()
            && items.len() == 2
            && items[0]
                .symbol_reference()
                .is_some_and(|(name, _)| normalize_name(name) == "EQL")
        {
            return Ok(MethodSpecializer::Eql(items[1].clone()));
        }
        Err(RuntimeError::Type {
            expected: "CLASS".to_owned(),
            actual: value.type_name().to_string(),
            span: Some(span),
        })
    }

    fn same_method_specializer(&self, left: &MethodSpecializer, right: &MethodSpecializer) -> bool {
        match (left, right) {
            (MethodSpecializer::Class(left), MethodSpecializer::Class(right)) => left == right,
            (MethodSpecializer::Eql(left), MethodSpecializer::Eql(right)) => {
                builtins::eql_value(left, right)
            }
            _ => false,
        }
    }

    fn compute_applicable_methods(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 2 {
            return Err(self.arity("compute-applicable-methods", "two", arguments.len()));
        }
        let function = self.resolve_function_designator(&arguments[0], span, environment)?;
        let method_arguments = arguments[1].list_items().ok_or_else(|| {
            self.invalid(
                "compute-applicable-methods arguments must be a proper list",
                span,
            )
        })?;
        let crate::Function::Generic { methods, .. } = function.as_ref() else {
            return Err(RuntimeError::Type {
                expected: "GENERIC-FUNCTION".to_owned(),
                actual: Value::Function(function).type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Value::list(
            self.ordered_applicable_methods(methods, &method_arguments)
                .into_iter()
                .map(|method| Value::Method(Rc::new(method)))
                .collect(),
        ))
    }

    fn generic_function_methods(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("generic-function-methods", "one", arguments.len()));
        }
        let function = self.resolve_function_designator(&arguments[0], span, environment)?;
        let crate::Function::Generic { methods, .. } = function.as_ref() else {
            return Err(RuntimeError::Type {
                expected: "GENERIC-FUNCTION".to_owned(),
                actual: Value::Function(function).type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Value::list(
            methods
                .borrow()
                .iter()
                .cloned()
                .map(|method| Value::Method(Rc::new(method)))
                .collect(),
        ))
    }

    fn generic_function_name(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("generic-function-name", "one", arguments.len()));
        }
        let function = self.resolve_function_designator(&arguments[0], span, environment)?;
        let crate::Function::Generic { name, .. } = function.as_ref() else {
            return Err(RuntimeError::Type {
                expected: "GENERIC-FUNCTION".to_owned(),
                actual: Value::Function(function).type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Value::symbol(name.clone()))
    }

    fn generic_function_class(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("generic-function-class", "one", arguments.len()));
        }
        let function = self.resolve_function_designator(&arguments[0], span, environment)?;
        let crate::Function::Generic { .. } = function.as_ref() else {
            return Err(RuntimeError::Type {
                expected: "GENERIC-FUNCTION".to_owned(),
                actual: Value::Function(function).type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Self::class_object_named(
            "STANDARD-GENERIC-FUNCTION",
            environment,
        ))
    }

    fn method_class(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-class", "one", arguments.len()));
        }
        let Value::Method(_) = &arguments[0] else {
            return Err(RuntimeError::Type {
                expected: "METHOD".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Self::class_object_named("STANDARD-METHOD", environment))
    }

    fn method_combination(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-combination", "one", arguments.len()));
        }
        let function = self.resolve_function_designator(&arguments[0], span, environment)?;
        let crate::Function::Generic { .. } = function.as_ref() else {
            return Err(RuntimeError::Type {
                expected: "GENERIC-FUNCTION".to_owned(),
                actual: Value::Function(function).type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Value::symbol("STANDARD"))
    }

    fn method_qualifiers(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-qualifiers", "one", arguments.len()));
        }
        let Value::Method(method) = &arguments[0] else {
            return Err(RuntimeError::Type {
                expected: "METHOD".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Value::list(
            method.qualifiers.iter().map(Value::keyword).collect(),
        ))
    }

    fn method_function(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-function", "one", arguments.len()));
        }
        let Value::Method(method) = &arguments[0] else {
            return Err(RuntimeError::Type {
                expected: "METHOD".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(method.function.clone())
    }

    fn method_generic_function(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-generic-function", "one", arguments.len()));
        }
        let Value::Method(method) = &arguments[0] else {
            return Err(RuntimeError::Type {
                expected: "METHOD".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        let Some(Value::Function(function)) = environment.lookup_function(&method.generic_function)
        else {
            return Ok(Value::Nil);
        };
        match function.as_ref() {
            crate::Function::Generic { .. } => Ok(Value::Function(function)),
            _ => Ok(Value::Nil),
        }
    }

    fn method_lambda_list(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-lambda-list", "one", arguments.len()));
        }
        let Value::Method(method) = &arguments[0] else {
            return Err(RuntimeError::Type {
                expected: "METHOD".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(method.lambda_list.clone())
    }

    fn method_specializers(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-specializers", "one", arguments.len()));
        }
        let Value::Method(method) = &arguments[0] else {
            return Err(RuntimeError::Type {
                expected: "METHOD".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Value::list(
            method
                .specializers
                .iter()
                .map(|specializer| self.method_specializer_value(specializer, environment))
                .collect(),
        ))
    }

    fn method_specializer_value(
        &self,
        specializer: &MethodSpecializer,
        environment: &Environment,
    ) -> Value {
        match specializer {
            MethodSpecializer::Class(class_name) => environment
                .lookup_class(class_name)
                .map(Value::class_object)
                .unwrap_or_else(|| {
                    Value::class_object(Rc::new(ClassDefinition {
                        name: class_name.clone(),
                        precedence: vec![class_name.clone(), "STANDARD-OBJECT".to_owned()],
                        slots: Vec::new(),
                        default_initargs: Vec::new(),
                        documentation: Rc::new(RefCell::new(None)),
                    }))
                }),
            MethodSpecializer::Eql(value) => Value::list(vec![Value::symbol("EQL"), value.clone()]),
        }
    }

    fn class_object_named(name: &str, environment: &Environment) -> Value {
        environment
            .lookup_class(name)
            .map(Value::class_object)
            .unwrap_or_else(|| {
                Value::class_object(Rc::new(ClassDefinition {
                    name: name.to_owned(),
                    precedence: vec![name.to_owned(), "STANDARD-OBJECT".to_owned()],
                    slots: Vec::new(),
                    default_initargs: Vec::new(),
                    documentation: Rc::new(RefCell::new(None)),
                }))
            })
    }

    fn special_defmethod(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("defmethod", "three", items.len().saturating_sub(1)));
        }
        let name = self.variable_name(&items[1], "defmethod name must be a symbol")?;
        let name = unqualified_name(&name);
        let lambda_index = items[2..]
            .iter()
            .position(|form| matches!(form.kind, FormKind::List(_)))
            .map(|index| index + 2)
            .ok_or_else(|| {
                self.invalid("defmethod requires a method lambda list", items[1].span)
            })?;

        let qualifiers = items[2..lambda_index]
            .iter()
            .map(|form| {
                let qualifier = self.definition_name_from_form(form, "defmethod qualifier")?;
                match qualifier.as_str() {
                    "BEFORE" | "AFTER" | "AROUND" => Ok(qualifier),
                    _ => Err(self.invalid("unsupported defmethod qualifier", form.span)),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let FormKind::List(parameters) = &items[lambda_index].kind else {
            return Err(self.invalid(
                "defmethod lambda list must be a list",
                items[lambda_index].span,
            ));
        };

        let mut required = Vec::new();
        let mut required_escaped = Vec::new();
        let mut specializers = Vec::new();
        let mut normalized_parameters = Vec::new();
        let mut required_parameter_count = 0;
        for parameter in parameters {
            if matches!(&parameter.kind, FormKind::Atom(name) if normalize_name(name).starts_with('&'))
            {
                break;
            }
            let (name_form, specializer_form) = match &parameter.kind {
                FormKind::Atom(_) => (parameter, None),
                FormKind::List(parts) if (1..=2).contains(&parts.len()) => {
                    (&parts[0], parts.get(1))
                }
                _ => {
                    return Err(self.invalid(
                        "defmethod parameter must be a variable or (variable class)",
                        parameter.span,
                    ));
                }
            };
            let (parameter_name, escaped) =
                self.variable_name_info(name_form, "defmethod parameter must be a variable")?;
            required.push(unqualified_name(&parameter_name));
            required_escaped.push(escaped);
            let specializer = match specializer_form {
                None => MethodSpecializer::Class("T".to_owned()),
                Some(form) => {
                    let is_eql = matches!(&form.kind, FormKind::List(parts) if parts.len() == 2
                        && atom_name(&parts[0]).is_some_and(|name| normalize_name(name) == "EQL"));
                    if is_eql {
                        let FormKind::List(parts) = &form.kind else {
                            unreachable!();
                        };
                        MethodSpecializer::Eql(self.eval_in(&parts[1], environment)?)
                    } else {
                        let class =
                            self.definition_name_from_form(form, "defmethod specializer")?;
                        if class != "T"
                            && class != "OBJECT"
                            && class != "STANDARD-OBJECT"
                            && environment.lookup_class(&class).is_none()
                        {
                            return Err(
                                self.invalid("unknown defmethod specializer", parameter.span)
                            );
                        }
                        MethodSpecializer::Class(class)
                    }
                }
            };
            specializers.push(specializer);
            normalized_parameters.push(name_form.clone());
            required_parameter_count += 1;
        }
        normalized_parameters.extend(
            parameters
                .get(required_parameter_count..)
                .unwrap_or_default()
                .iter()
                .cloned(),
        );
        let normalized_lambda_list = Form::list(normalized_parameters, items[lambda_index].span);
        let lambda_list = self.parameters(&normalized_lambda_list)?;

        let generic = environment.lookup_function(&name).or_else(|| {
            let generic = Value::generic(name.clone(), lambda_list.clone());
            environment.define_function(&name, generic.clone());
            Some(generic)
        });
        let Some(Value::Function(generic)) = generic else {
            return Err(self.invalid("defmethod name is not a generic function", items[1].span));
        };
        let crate::Function::Generic {
            methods,
            lambda_list: generic_lambda_list,
            ..
        } = generic.as_ref()
        else {
            return Err(self.invalid("defmethod name is not a generic function", items[1].span));
        };
        self.ensure_generic_lambda_list_congruence(
            generic_lambda_list,
            &lambda_list,
            items[lambda_index].span,
        )?;
        let closure = Value::closure_with_keywords(ClosureData {
            parameters: required,
            required_escaped,
            optional: lambda_list.optional,
            rest: lambda_list.rest,
            rest_escaped: lambda_list.rest_escaped,
            keywords: lambda_list.keywords,
            has_keyword_section: lambda_list.has_keyword_section,
            allow_other_keys: lambda_list.allow_other_keys,
            auxiliary: lambda_list.auxiliary,
            body: items[lambda_index + 1..].to_vec(),
            environment: environment.clone(),
        });
        let definition = MethodDefinition {
            id: self.fresh_method_id(),
            generic_function: name.clone(),
            lambda_list: self.quoted_value(&normalized_lambda_list)?,
            qualifiers,
            specializers,
            function: closure,
        };
        let mut methods = methods.borrow_mut();
        if let Some(existing) = methods
            .iter_mut()
            .find(|method| self.same_method_identity(method, &definition))
        {
            *existing = definition;
        } else {
            methods.push(definition);
        }
        Ok(Value::symbol(name))
    }

    fn ensure_generic_lambda_list_congruence(
        &self,
        generic: &OrdinaryLambdaList,
        method: &OrdinaryLambdaList,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if generic.required.len() != method.required.len() {
            return Err(self.invalid(
                "defmethod lambda list is not congruent with generic function",
                span,
            ));
        }
        if !generic.optional.is_empty() && generic.optional.len() != method.optional.len() {
            return Err(self.invalid(
                "defmethod lambda list is not congruent with generic function",
                span,
            ));
        }
        if generic.has_keyword_section {
            if !method.has_keyword_section {
                return Err(self.invalid(
                    "defmethod lambda list is not congruent with generic function",
                    span,
                ));
            }
            if generic.allow_other_keys && !method.allow_other_keys {
                return Err(self.invalid(
                    "defmethod lambda list is not congruent with generic function",
                    span,
                ));
            }
            let method_keywords = method
                .keywords
                .iter()
                .map(|parameter| normalize_name(&parameter.keyword_name))
                .collect::<HashSet<_>>();
            if generic
                .keywords
                .iter()
                .map(|parameter| normalize_name(&parameter.keyword_name))
                .any(|keyword| !method_keywords.contains(&keyword))
            {
                return Err(self.invalid(
                    "defmethod lambda list is not congruent with generic function",
                    span,
                ));
            }
        }
        Ok(())
    }

    fn list_form_items<'a>(
        &self,
        form: &'a Form,
        context: &str,
    ) -> Result<&'a [Form], RuntimeError> {
        match &form.kind {
            FormKind::List(items) => Ok(items),
            FormKind::Atom(name) if normalize_name(name) == "NIL" => Ok(&[]),
            _ => Err(self.invalid(context, form.span)),
        }
    }

    fn definition_name_from_form(
        &self,
        form: &Form,
        context: &str,
    ) -> Result<String, RuntimeError> {
        let Some(raw_name) = atom_name(form) else {
            return Err(self.invalid(context, form.span));
        };
        let token = parse_symbol_token(raw_name).map_err(|_| self.invalid(context, form.span))?;
        if !matches!(
            token.kind,
            SymbolTokenKind::Symbol | SymbolTokenKind::Keyword
        ) || token.name.is_empty()
        {
            return Err(self.invalid(context, form.span));
        }
        if token.escaped && token.package.is_some() {
            return Err(self.invalid(context, form.span));
        }
        let normalized = if token.escaped {
            token.name
        } else {
            normalize_name(raw_name)
        };
        Ok(unqualified_name(normalized.trim_start_matches(':')))
    }

    fn defstruct_name_option(
        &self,
        option_form: &Form,
        option_items: &[Form],
        default_name: String,
        context: &str,
    ) -> Result<Option<String>, RuntimeError> {
        if option_items.len() > 2 {
            return Err(self.invalid(
                "defstruct naming options accept at most one name",
                option_form.span,
            ));
        }
        let Some(name_form) = option_items.get(1) else {
            return Ok(Some(default_name));
        };
        if is_nil_form(name_form) {
            return Ok(None);
        }
        let (raw_name, _) = self.variable_name_info(name_form, context)?;
        Ok(Some(unqualified_name(&raw_name)))
    }

    fn defstruct_constructor_option(
        &self,
        option_form: &Form,
        option_items: &[Form],
        default_name: String,
    ) -> Result<(Option<String>, Option<OrdinaryLambdaList>), RuntimeError> {
        if option_items.len() > 3 {
            return Err(self.invalid(
                "defstruct :constructor accepts at most a name and a lambda list",
                option_form.span,
            ));
        }
        let constructor_name = match option_items.get(1) {
            None => Some(default_name),
            Some(name_form) if is_nil_form(name_form) => None,
            Some(name_form) => {
                let (raw_name, _) = self.variable_name_info(
                    name_form,
                    "defstruct :constructor must name a symbol or NIL",
                )?;
                Some(unqualified_name(&raw_name))
            }
        };
        let constructor_lambda_list = option_items
            .get(2)
            .map(|lambda_list_form| {
                if constructor_name.is_none() {
                    return Err(self.invalid(
                        "defstruct :constructor NIL cannot have a lambda list",
                        lambda_list_form.span,
                    ));
                }
                self.parameters(lambda_list_form)
            })
            .transpose()?;
        Ok((constructor_name, constructor_lambda_list))
    }

    fn defstruct_slot_description(
        &self,
        slot_form: &Form,
        environment: &Environment,
    ) -> Result<(String, Option<Form>, Option<bool>), RuntimeError> {
        match &slot_form.kind {
            FormKind::Atom(_) => Ok((
                self.variable_name_info(
                    slot_form,
                    "defstruct slot must be a symbol or a slot specification",
                )?
                .0,
                None,
                None,
            )),
            FormKind::List(slot_items) if (1..=3).contains(&slot_items.len()) => {
                let slot_name = self
                    .variable_name_info(&slot_items[0], "defstruct slot name must be a symbol")?;
                let read_only = slot_items
                    .get(2)
                    .map(|form| {
                        self.eval_in(form, environment)
                            .map(|value| value.is_truthy())
                    })
                    .transpose()?;
                Ok((slot_name.0, slot_items.get(1).cloned(), read_only))
            }
            _ => Err(self.invalid(
                "defstruct slot must be a symbol or a one- to three-element list",
                slot_form.span,
            )),
        }
    }


}
