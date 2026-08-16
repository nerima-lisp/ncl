impl Runtime {
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

}
