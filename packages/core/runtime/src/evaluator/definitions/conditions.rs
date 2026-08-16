impl Runtime {
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

}
