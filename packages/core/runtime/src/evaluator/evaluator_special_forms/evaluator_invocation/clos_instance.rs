use super::{Environment, Runtime, RuntimeError, Span, Value, quoted_form_value};

impl Runtime {
    pub(crate) fn change_class(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 2 {
            return Err(Self::arity("change-class", "two", arguments.len()));
        }
        let class_name = Self::name_designator_from_value(&arguments[1], span)?;
        let class = environment
            .lookup_class(&class_name)
            .ok_or_else(|| Self::invalid("unknown class", span))?;
        if !arguments[0].change_instance_class(class) {
            return Err(Self::invalid("change-class requires an instance", span));
        }
        Ok(arguments[0].clone())
    }

    pub(crate) fn allocate_instance(
        &self,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("allocate-instance", "one", arguments.len()));
        }
        let Some(class) = arguments[0].class_definition() else {
            return Err(Self::invalid("allocate-instance requires a class", span));
        };
        let slots = class
            .slots
            .iter()
            .map(|slot| (slot.name.clone(), Value::Unbound))
            .collect();
        Ok(Value::instance(class, slots))
    }

    pub(crate) fn set_instance_slot_checked(
        &self,
        instance: &Value,
        class_name: &str,
        slot_name: &str,
        value: Value,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let Some(class) = instance.instance_class_definition() else {
            return Err(Self::invalid("slot target is not an instance", span));
        };
        let Some(slot) = class
            .slots
            .iter()
            .find(|slot| slot.name.eq_ignore_ascii_case(slot_name))
        else {
            return Err(Self::invalid("slot is not defined for this class", span));
        };
        if let Some(type_form) = &slot.type_form {
            let type_designator = quoted_form_value(type_form)?;
            if !crate::builtins::typep_value(&value, &type_designator)? {
                return Err(Self::invalid(
                    "slot value does not satisfy declared type",
                    span,
                ));
            }
        }
        if instance.set_instance_slot(class_name, slot_name, value) {
            Ok(())
        } else {
            Err(Self::invalid("slot is not defined for this class", span))
        }
    }

    pub(crate) fn make_instance(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(Self::arity(
                "make-instance",
                "at least one",
                arguments.len(),
            ));
        }
        if !(arguments.len() - 1).is_multiple_of(2) {
            return Err(Self::invalid("make-instance initargs require pairs", span));
        }
        let class_name = Self::name_designator_from_value(&arguments[0], span)?;
        let class = environment
            .lookup_class(&class_name)
            .ok_or_else(|| Self::invalid("unknown class", span))?;

        let mut initargs = Vec::with_capacity(arguments.len().saturating_sub(1));
        for pair in arguments[1..].as_chunks::<2>().0 {
            let initarg = Self::name_designator_from_value(&pair[0], span)?;
            initargs.push((initarg, pair[1].clone()));
        }
        for (initarg, init_form) in &class.default_initargs {
            if initargs.iter().any(|(name, _)| name == initarg) {
                continue;
            }
            initargs.push((initarg.clone(), self.eval_in(init_form, environment)?));
        }
        for (initarg, _) in &initargs {
            if !class
                .slots
                .iter()
                .any(|slot| slot.initargs.iter().any(|name| name == initarg))
            {
                return Err(Self::invalid("unknown make-instance initarg", span));
            }
        }

        let instance = self.allocate_instance(&[Value::class_object(class.clone())], span)?;
        for slot in &class.slots {
            if slot
                .class_value
                .as_ref()
                .is_some_and(|value| !matches!(*value.borrow(), Value::Unbound))
            {
                continue;
            }
            let Some(function) = &slot.init_function else {
                continue;
            };
            let value = self.apply_in(function, &[], span, environment)?;
            self.set_instance_slot_checked(&instance, &class.name, &slot.name, value, span)?;
        }
        let mut initialize_arguments = vec![instance.clone()];
        for (initarg, value) in &initargs {
            let Some(index) = class
                .slots
                .iter()
                .position(|slot| slot.initargs.iter().any(|name| name == initarg))
            else {
                return Err(Self::invalid("unknown make-instance initarg", span));
            };
            self.set_instance_slot_checked(
                &instance,
                &class.name,
                &class.slots[index].name,
                value.clone(),
                span,
            )?;
            initialize_arguments.push(Value::keyword(initarg.clone()));
            initialize_arguments.push(value.clone());
        }
        self.apply_in(
            &Value::symbol("initialize-instance"),
            &initialize_arguments,
            span,
            environment,
        )
    }

    pub(crate) fn reinitialize_instance(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(Self::arity(
                "reinitialize-instance",
                "at least one",
                arguments.len(),
            ));
        }
        if !(arguments.len() - 1).is_multiple_of(2) {
            return Err(Self::invalid(
                "reinitialize-instance initargs require pairs",
                span,
            ));
        }
        let instance = &arguments[0];
        let Some(class) = instance.instance_class_definition() else {
            return Err(Self::invalid(
                "reinitialize-instance requires an instance",
                span,
            ));
        };
        for pair in arguments[1..].as_chunks::<2>().0 {
            let initarg = Self::name_designator_from_value(&pair[0], span)?;
            if !class
                .slots
                .iter()
                .any(|slot| slot.initargs.iter().any(|name| name == &initarg))
            {
                return Err(Self::invalid("unknown reinitialize-instance initarg", span));
            }
        }
        let mut shared = vec![instance.clone(), Value::Nil];
        shared.extend_from_slice(&arguments[1..]);
        let function = environment
            .lookup_function("SHARED-INITIALIZE")
            .unwrap_or_else(|| Value::primitive("SHARED-INITIALIZE"));
        self.apply_in(&function, &shared, span, environment)
    }

    pub(crate) fn initialize_instance(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(Self::arity("initialize-instance", "at least one", 0));
        }
        let mut shared = vec![arguments[0].clone(), Value::symbol("T")];
        shared.extend_from_slice(&arguments[1..]);
        let function = environment
            .lookup_function("SHARED-INITIALIZE")
            .unwrap_or_else(|| Value::primitive("SHARED-INITIALIZE"));
        self.apply_in(&function, &shared, span, environment)
    }

    pub(crate) fn shared_initialize(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() < 2 {
            return Err(Self::arity(
                "shared-initialize",
                "at least two",
                arguments.len(),
            ));
        }
        let instance = &arguments[0];
        let Some(class) = instance.instance_class_definition() else {
            return Err(Self::invalid(
                "shared-initialize requires an instance",
                span,
            ));
        };
        for pair in arguments[2..].as_chunks::<2>().0 {
            let initarg = Self::name_designator_from_value(&pair[0], span)?;
            let Some(slot) = class
                .slots
                .iter()
                .find(|slot| slot.initargs.iter().any(|name| name == &initarg))
            else {
                return Err(Self::invalid("unknown shared-initialize initarg", span));
            };
            self.set_instance_slot_checked(
                instance,
                &class.name,
                &slot.name,
                pair[1].clone(),
                span,
            )?;
        }
        let _ = environment;
        Ok(instance.clone())
    }
}
