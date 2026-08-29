#![allow(clippy::wildcard_imports)]
use super::*;

struct DefmethodParameters {
    required: Vec<String>,
    required_escaped: Vec<bool>,
    specializers: Vec<String>,
    normalized: Vec<Form>,
    required_count: usize,
}

struct DefstructRegistration {
    structure_name: String,
    structure_types: Vec<String>,
    slots: Vec<StructureSlot>,
    constructor_options: Vec<(Option<String>, Option<OrdinaryLambdaList>)>,
    predicate_name: Option<String>,
    copier_name: Option<String>,
    conc_name: String,
}

struct DefstructOptions {
    conc_name: String,
    predicate_name: Option<String>,
    copier_name: Option<String>,
    constructor_options: Vec<(Option<String>, Option<OrdinaryLambdaList>)>,
    included_structure: Option<(StructureDefinition, Vec<Form>)>,
}

struct DefclassSlotRegistration {
    slot: ClassSlot,
    readers: Vec<(String, String)>,
    writers: Vec<(String, String)>,
}

impl Runtime {
    pub(crate) fn special_defvar(
        &self,
        items: &[Form],
        environment: &Environment,
        force: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if force { "defparameter" } else { "defvar" };
        if !(items.len() == 2 || items.len() == 3) {
            return Err(Self::arity(
                operator,
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let context = if force {
            "defparameter name must be a symbol"
        } else {
            "defvar name must be a symbol"
        };
        let (name, escaped) = Self::variable_name_info(&items[1], context)?;
        if force
            && if escaped {
                self.is_constant_exact_in(&name)
            } else {
                self.is_constant_in(&name)
            }
        {
            return Err(Self::constant_modification_error(&name, items[1].span));
        }
        if !force {
            let existing = if escaped {
                self.lookup_special_exact(&name)
            } else {
                self.lookup_special(&name)
            };
            if let Some(value) = existing {
                return Ok(value);
            }
        }
        let value = items
            .get(2)
            .map_or(Ok(Value::Nil), |form| self.eval_in(form, environment))?;
        Ok(if escaped {
            self.define_special_value_exact(&name, value, force)
        } else {
            self.define_special_value(&name, value, force)
        })
    }

    pub(crate) fn special_defconstant(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(Self::arity(
                "defconstant",
                "two or three",
                items.len().saturating_sub(1),
            ));
        }
        let (name, escaped) =
            Self::variable_name_info(&items[1], "defconstant name must be a symbol")?;
        if if escaped {
            self.is_constant_exact_in(&name)
        } else {
            self.is_constant_in(&name)
        } {
            return Err(Self::constant_modification_error(&name, items[1].span));
        }
        let value = self.eval_in(&items[2], environment)?;
        Ok(if escaped {
            self.define_constant_value_exact(&name, value)
        } else {
            self.define_constant_value(&name, value)
        })
    }

    pub(crate) fn special_defstruct(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "defstruct",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let (name_form, option_forms, slot_forms) = match &items[1].kind {
            FormKind::Atom(_) => (&items[1], &items[2..2], &items[2..]),
            FormKind::List(name_and_options) if !name_and_options.is_empty() => {
                (&name_and_options[0], &name_and_options[1..], &items[2..])
            }
            _ => {
                return Err(Self::invalid(
                    "defstruct name must be a symbol or a name-and-options list",
                    items[1].span,
                ));
            }
        };
        let (raw_name, _) = Self::variable_name_info(name_form, "defstruct name must be a symbol")?;
        let structure_name = unqualified_name(&raw_name);
        let options = Self::parse_defstruct_options(&structure_name, option_forms, environment)?;
        let DefstructOptions {
            conc_name,
            predicate_name,
            copier_name,
            constructor_options,
            included_structure,
        } = options;
        let mut structure_types = vec![structure_name.clone()];
        let mut slots = Vec::new();
        let mut slot_names = HashSet::new();
        if let Some((parent, overrides)) = included_structure {
            structure_types.extend(parent.type_names.clone());
            slots = parent.slots;
            for slot in &slots {
                slot_names.insert(slot.name.clone());
            }
            let mut overridden_slots = HashSet::new();
            for slot_form in overrides {
                let (raw_slot_name, init_form, read_only) =
                    self.defstruct_slot_description(&slot_form, environment)?;
                let slot_name = unqualified_name(&raw_slot_name);
                let Some(slot) = slots.iter_mut().find(|slot| slot.name == slot_name) else {
                    return Err(Self::invalid(
                        "defstruct :include slot must name an inherited slot",
                        slot_form.span,
                    ));
                };
                if !overridden_slots.insert(slot_name) {
                    return Err(Self::invalid(
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
                return Err(Self::invalid(
                    "defstruct cannot define duplicate slots",
                    slot_form.span,
                ));
            }
            slots.push(StructureSlot {
                name: slot_name,
                init_form,
                read_only: read_only.unwrap_or(false),
            });
        }

        Self::register_defstruct(
            environment,
            DefstructRegistration {
                structure_name: structure_name.clone(),
                structure_types,
                slots,
                constructor_options,
                predicate_name,
                copier_name,
                conc_name,
            },
        );
        Ok(Value::symbol(structure_name))
    }

    fn parse_defstruct_options(
        structure_name: &str,
        option_forms: &[Form],
        environment: &Environment,
    ) -> Result<DefstructOptions, RuntimeError> {
        let mut conc_name = format!("{structure_name}-");
        let mut predicate_name = Some(format!("{structure_name}-P"));
        let mut copier_name = Some(format!("COPY-{structure_name}"));
        let mut constructor_options: Vec<(Option<String>, Option<OrdinaryLambdaList>)> = Vec::new();
        let mut seen_options = HashSet::new();
        let mut included_structure: Option<(StructureDefinition, Vec<Form>)> = None;
        for option_form in option_forms {
            let FormKind::List(option_items) = &option_form.kind else {
                return Err(Self::invalid(
                    "defstruct option must be a list",
                    option_form.span,
                ));
            };
            let Some(option_name) = option_items.first().and_then(atom_name) else {
                return Err(Self::invalid(
                    "defstruct option needs a name",
                    option_form.span,
                ));
            };
            let normalized_option = normalize_name(option_name);
            let option_name = normalized_option.trim_start_matches(':');
            Self::check_unique_defstruct_option(option_name, &mut seen_options, option_form.span)?;
            match option_name {
                "CONC-NAME" => {
                    conc_name = Self::defstruct_name_option(
                        option_form,
                        option_items,
                        format!("{structure_name}-"),
                        "defstruct :conc-name must name a symbol or NIL",
                    )?
                    .unwrap_or_default();
                }
                "PREDICATE" => {
                    predicate_name = Self::defstruct_name_option(
                        option_form,
                        option_items,
                        format!("{structure_name}-P"),
                        "defstruct :predicate must name a symbol or NIL",
                    )?;
                }
                "COPIER" => {
                    copier_name = Self::defstruct_name_option(
                        option_form,
                        option_items,
                        format!("COPY-{structure_name}"),
                        "defstruct :copier must name a symbol or NIL",
                    )?;
                }
                "INCLUDE" => {
                    if option_items.len() < 2 {
                        return Err(Self::invalid(
                            "defstruct :include needs a structure name",
                            option_form.span,
                        ));
                    }
                    let (raw_parent_name, _) = Self::variable_name_info(
                        &option_items[1],
                        "defstruct :include structure name must be a symbol",
                    )?;
                    let parent_name = unqualified_name(&raw_parent_name);
                    let Some(parent) = environment.lookup_structure(&parent_name) else {
                        return Err(Self::invalid(
                            "defstruct :include requires a previously defined structure",
                            option_form.span,
                        ));
                    };
                    included_structure = Some((parent, option_items[2..].to_vec()));
                }
                "CONSTRUCTOR" => {
                    let constructor = Self::defstruct_constructor_option(
                        option_form,
                        option_items,
                        format!("MAKE-{structure_name}"),
                    )?;
                    if (constructor.0.is_none() && !constructor_options.is_empty())
                        || constructor_options.iter().any(|(name, _)| name.is_none())
                    {
                        return Err(Self::invalid(
                            "defstruct :constructor NIL cannot be combined with another constructor",
                            option_form.span,
                        ));
                    }
                    constructor_options.push(constructor);
                }
                _ => {
                    return Err(Self::invalid(
                        "unsupported defstruct option",
                        option_items[0].span,
                    ));
                }
            }
        }
        Ok(DefstructOptions {
            conc_name,
            predicate_name,
            copier_name,
            constructor_options,
            included_structure,
        })
    }

    fn check_unique_defstruct_option(
        option_name: &str,
        seen_options: &mut HashSet<String>,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if option_name != "CONSTRUCTOR" && !seen_options.insert(option_name.to_string()) {
            return Err(Self::invalid("defstruct cannot repeat an option", span));
        }
        Ok(())
    }

    fn register_defstruct(environment: &Environment, registration: DefstructRegistration) {
        let DefstructRegistration {
            structure_name,
            structure_types,
            slots,
            mut constructor_options,
            predicate_name,
            copier_name,
            conc_name,
        } = registration;
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
    }

    pub(crate) fn special_defclass(
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(Self::arity(
                "defclass",
                "four",
                items.len().saturating_sub(1),
            ));
        }

        let class_name = Self::variable_name(&items[1], "defclass name must be a symbol")?;
        let class_name = unqualified_name(&class_name);
        let superclasses = Self::list_form_items(&items[2], "defclass superclass list")?;
        let mut direct_superclasses = Vec::with_capacity(superclasses.len());
        for superclass in superclasses {
            let name = Self::definition_name_from_form(superclass, "defclass superclass")?;
            if !direct_superclasses.contains(&name) {
                direct_superclasses.push(name);
            }
        }

        let slot_forms = Self::list_form_items(&items[3], "defclass slot list")?;
        let mut slots: Vec<ClassSlot> = Vec::new();
        let mut readers = Vec::new();
        let mut writers = Vec::new();
        let mut default_initargs = Vec::new();

        for slot_form in slot_forms {
            let registration = Self::parse_defclass_slot(slot_form)?;
            let slot_name = registration.slot.name.clone();
            readers.extend(registration.readers);
            writers.extend(registration.writers);
            if let Some(existing) = slots.iter_mut().find(|slot| slot.name == slot_name) {
                *existing = registration.slot;
            } else {
                slots.push(registration.slot);
            }
        }

        for option in items.iter().skip(4) {
            Self::parse_defclass_option(option, &mut default_initargs)?;
        }

        let precedence = Self::merge_defclass_superclasses(
            &class_name,
            &direct_superclasses,
            &mut slots,
            &mut default_initargs,
            environment,
            items[2].span,
        )?;

        let definition = Rc::new(ClassDefinition {
            name: class_name.clone(),
            precedence,
            slots,
            default_initargs,
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

    fn parse_defclass_option(
        option: &Form,
        default_initargs: &mut Vec<(String, Form)>,
    ) -> Result<(), RuntimeError> {
        let option_items = Self::list_form_items(option, "defclass option")?;
        if option_items.is_empty() {
            return Err(Self::invalid(
                "defclass option must be a non-empty list",
                option.span,
            ));
        }
        let option_name =
            Self::definition_name_from_form(&option_items[0], "defclass option name")?;
        match option_name.as_str() {
            "DEFAULT-INITARGS" => {
                if option_items.len() < 3 || !(option_items.len() - 1).is_multiple_of(2) {
                    return Err(Self::invalid(
                        "defclass :default-initargs requires initarg and form pairs",
                        option.span,
                    ));
                }
                for pair in option_items[1..].as_chunks::<2>().0 {
                    let initarg =
                        Self::definition_name_from_form(&pair[0], "defclass default initarg")?;
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
            "DOCUMENTATION"
                if option_items.len() != 2
                    || !matches!(option_items[1].kind, FormKind::String(_)) =>
            {
                return Err(Self::invalid(
                    "defclass :documentation needs one string",
                    option.span,
                ));
            }
            _ => {}
        }
        Ok(())
    }

    fn merge_defclass_superclasses(
        class_name: &str,
        direct_superclasses: &[String],
        slots: &mut Vec<ClassSlot>,
        default_initargs: &mut Vec<(String, Form)>,
        environment: &Environment,
        span: Span,
    ) -> Result<Vec<String>, RuntimeError> {
        let mut precedence = vec![class_name.to_owned()];
        for superclass in direct_superclasses {
            if superclass == "OBJECT" || superclass == "STANDARD-OBJECT" {
                if !precedence.iter().any(|name| name == "STANDARD-OBJECT") {
                    precedence.push("STANDARD-OBJECT".to_owned());
                }
                continue;
            }
            let Some(definition) = environment.lookup_class(superclass) else {
                return Err(Self::invalid("unknown defclass superclass", span));
            };
            for name in &definition.precedence {
                if !precedence.iter().any(|existing| existing == name) {
                    precedence.push(name.clone());
                }
            }
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
        if !precedence.iter().any(|name| name == "STANDARD-OBJECT") {
            precedence.push("STANDARD-OBJECT".to_owned());
        }
        Ok(precedence)
    }

    fn parse_defclass_slot(slot_form: &Form) -> Result<DefclassSlotRegistration, RuntimeError> {
        let (slot_name_form, options) = match &slot_form.kind {
            FormKind::Atom(_) => (slot_form, &[][..]),
            FormKind::List(slot_items) if !slot_items.is_empty() => {
                (&slot_items[0], &slot_items[1..])
            }
            _ => {
                return Err(Self::invalid(
                    "defclass slot must be a symbol or non-empty list",
                    slot_form.span,
                ));
            }
        };
        let slot_name = unqualified_name(&Self::variable_name(
            slot_name_form,
            "defclass slot must be a symbol",
        )?);
        let mut initarg = None;
        let mut init_form = None;
        let mut class_value = None;
        let mut readers = Vec::new();
        let mut writers = Vec::new();
        if !options.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "defclass slot options require a value",
                slot_form.span,
            ));
        }
        for option in options.as_chunks::<2>().0 {
            let option_name = Self::definition_name_from_form(&option[0], "defclass slot option")?;
            match option_name.as_str() {
                "INITARG" => {
                    initarg = (!is_nil_form(&option[1]))
                        .then(|| Self::definition_name_from_form(&option[1], "defclass initarg"))
                        .transpose()?;
                }
                "INITFORM" => init_form = Some(option[1].clone()),
                "ALLOCATION" => {
                    let allocation =
                        Self::definition_name_from_form(&option[1], "defclass allocation")?;
                    class_value =
                        (allocation == "CLASS").then(|| Rc::new(RefCell::new(Value::Unbound)));
                }
                "ACCESSOR" | "READER" => {
                    let accessor_name =
                        Self::variable_name(&option[1], "defclass accessor must be a symbol")?;
                    readers.push((unqualified_name(&accessor_name), slot_name.clone()));
                }
                "WRITER" => {
                    let writer_name =
                        Self::variable_name(&option[1], "defclass writer must be a symbol")?;
                    writers.push((unqualified_name(&writer_name), slot_name.clone()));
                }
                "TYPE" | "DOCUMENTATION" => {}
                _ => {
                    return Err(Self::invalid(
                        "unsupported defclass slot option",
                        option[0].span,
                    ));
                }
            }
        }
        Ok(DefclassSlotRegistration {
            slot: ClassSlot {
                name: slot_name,
                initarg,
                init_form,
                class_value,
            },
            readers,
            writers,
        })
    }

    pub(crate) fn special_defgeneric(
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "defgeneric",
                "three",
                items.len().saturating_sub(1),
            ));
        }
        let name = Self::variable_name(&items[1], "defgeneric name must be a symbol")?;
        let name = unqualified_name(&name);
        let _ = Self::parameters(&items[2])?;
        environment.define_function(&name, Value::generic(name.clone()));
        Ok(Value::symbol(name))
    }

    pub(crate) fn special_defmethod(
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "defmethod",
                "three",
                items.len().saturating_sub(1),
            ));
        }
        let name = Self::variable_name(&items[1], "defmethod name must be a symbol")?;
        let name = unqualified_name(&name);
        let lambda_index = items[2..]
            .iter()
            .position(|form| matches!(form.kind, FormKind::List(_)))
            .map(|index| index + 2)
            .ok_or_else(|| {
                Self::invalid("defmethod requires a method lambda list", items[1].span)
            })?;

        let qualifiers = items[2..lambda_index]
            .iter()
            .map(|form| {
                let qualifier = Self::definition_name_from_form(form, "defmethod qualifier")?;
                match qualifier.as_str() {
                    "BEFORE" | "AFTER" | "AROUND" => Ok(qualifier),
                    _ => Err(Self::invalid("unsupported defmethod qualifier", form.span)),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let FormKind::List(parameters) = &items[lambda_index].kind else {
            return Err(Self::invalid(
                "defmethod lambda list must be a list",
                items[lambda_index].span,
            ));
        };

        let DefmethodParameters {
            required,
            required_escaped,
            specializers,
            mut normalized,
            required_count,
        } = Self::parse_defmethod_required_parameters(parameters, environment)?;
        normalized.extend(
            parameters
                .get(required_count..)
                .unwrap_or_default()
                .iter()
                .cloned(),
        );
        let normalized_lambda_list = Form::list(normalized, items[lambda_index].span);
        let lambda_list = Self::parameters(&normalized_lambda_list)?;

        let generic = environment.lookup_function(&name).or_else(|| {
            let generic = Value::generic(name.clone());
            environment.define_function(&name, generic.clone());
            Some(generic)
        });
        let Some(Value::Function(generic)) = generic else {
            return Err(Self::invalid(
                "defmethod name is not a generic function",
                items[1].span,
            ));
        };
        let crate::Function::Generic { methods, .. } = generic.as_ref() else {
            return Err(Self::invalid(
                "defmethod name is not a generic function",
                items[1].span,
            ));
        };
        let closure = Value::closure_with_keywords(
            crate::ClosureOptions {
                parameters: required,
                required_escaped,
                optional: lambda_list.optional,
                rest: lambda_list.rest,
                rest_escaped: lambda_list.rest_escaped,
                keywords: lambda_list.keywords,
                has_keyword_section: lambda_list.has_keyword_section,
                allow_other_keys: lambda_list.allow_other_keys,
                auxiliary: lambda_list.auxiliary,
            },
            items[lambda_index + 1..].to_vec(),
            environment.clone(),
        );
        methods.borrow_mut().push(MethodDefinition {
            qualifiers,
            specializers,
            function: closure,
        });
        Ok(Value::symbol(name))
    }

    fn parse_defmethod_required_parameters(
        parameters: &[Form],
        environment: &Environment,
    ) -> Result<DefmethodParameters, RuntimeError> {
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
                    return Err(Self::invalid(
                        "defmethod parameter must be a variable or (variable class)",
                        parameter.span,
                    ));
                }
            };
            let (parameter_name, escaped) =
                Self::variable_name_info(name_form, "defmethod parameter must be a variable")?;
            required.push(unqualified_name(&parameter_name));
            required_escaped.push(escaped);
            let specializer = match specializer_form {
                None => "T".to_owned(),
                Some(form) => Self::definition_name_from_form(form, "defmethod specializer")?,
            };
            if specializer != "T"
                && specializer != "OBJECT"
                && specializer != "STANDARD-OBJECT"
                && environment.lookup_class(&specializer).is_none()
            {
                return Err(Self::invalid(
                    "unknown defmethod specializer",
                    parameter.span,
                ));
            }
            specializers.push(specializer);
            normalized_parameters.push(name_form.clone());
            required_parameter_count += 1;
        }
        Ok(DefmethodParameters {
            required,
            required_escaped,
            specializers,
            normalized: normalized_parameters,
            required_count: required_parameter_count,
        })
    }

    fn list_form_items<'a>(form: &'a Form, context: &str) -> Result<&'a [Form], RuntimeError> {
        match &form.kind {
            FormKind::List(items) => Ok(items),
            FormKind::Atom(name) if normalize_name(name) == "NIL" => Ok(&[]),
            _ => Err(Self::invalid(context, form.span)),
        }
    }

    fn definition_name_from_form(form: &Form, context: &str) -> Result<String, RuntimeError> {
        let Some(raw_name) = atom_name(form) else {
            return Err(Self::invalid(context, form.span));
        };
        let token = parse_symbol_token(raw_name).map_err(|_| Self::invalid(context, form.span))?;
        if !matches!(
            token.kind,
            SymbolTokenKind::Symbol | SymbolTokenKind::Keyword
        ) || token.name.is_empty()
        {
            return Err(Self::invalid(context, form.span));
        }
        if token.escaped && token.package.is_some() {
            return Err(Self::invalid(context, form.span));
        }
        let normalized = if token.escaped {
            token.name
        } else {
            normalize_name(raw_name)
        };
        Ok(unqualified_name(normalized.trim_start_matches(':')))
    }

    fn defstruct_name_option(
        option_form: &Form,
        option_items: &[Form],
        default_name: String,
        context: &str,
    ) -> Result<Option<String>, RuntimeError> {
        if option_items.len() > 2 {
            return Err(Self::invalid(
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
        let (raw_name, _) = Self::variable_name_info(name_form, context)?;
        Ok(Some(unqualified_name(&raw_name)))
    }

    fn defstruct_constructor_option(
        option_form: &Form,
        option_items: &[Form],
        default_name: String,
    ) -> Result<(Option<String>, Option<OrdinaryLambdaList>), RuntimeError> {
        if option_items.len() > 3 {
            return Err(Self::invalid(
                "defstruct :constructor accepts at most a name and a lambda list",
                option_form.span,
            ));
        }
        let constructor_name = match option_items.get(1) {
            None => Some(default_name),
            Some(name_form) if is_nil_form(name_form) => None,
            Some(name_form) => {
                let (raw_name, _) = Self::variable_name_info(
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
                    return Err(Self::invalid(
                        "defstruct :constructor NIL cannot have a lambda list",
                        lambda_list_form.span,
                    ));
                }
                Self::parameters(lambda_list_form)
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
                Self::variable_name_info(
                    slot_form,
                    "defstruct slot must be a symbol or a slot specification",
                )?
                .0,
                None,
                None,
            )),
            FormKind::List(slot_items) if (1..=3).contains(&slot_items.len()) => {
                let slot_name = Self::variable_name_info(
                    &slot_items[0],
                    "defstruct slot name must be a symbol",
                )?;
                let read_only = slot_items
                    .get(2)
                    .map(|form| {
                        self.eval_in(form, environment)
                            .map(|value| value.is_truthy())
                    })
                    .transpose()?;
                Ok((slot_name.0, slot_items.get(1).cloned(), read_only))
            }
            _ => Err(Self::invalid(
                "defstruct slot must be a symbol or a one- to three-element list",
                slot_form.span,
            )),
        }
    }
}

impl Runtime {}
