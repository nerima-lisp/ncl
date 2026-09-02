use super::{Environment, Runtime, RuntimeError, Span, Value, quoted_form_value};

impl Runtime {
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
                .any(|slot| slot.initarg.as_deref() == Some(initarg.as_str()))
            {
                return Err(Self::invalid("unknown make-instance initarg", span));
            }
        }

        let mut slots = Vec::with_capacity(class.slots.len());
        for slot in &class.slots {
            let initarg_value = slot.initarg.as_ref().and_then(|initarg| {
                initargs
                    .iter()
                    .rev()
                    .find(|(name, _)| name == initarg)
                    .map(|(_, value)| value.clone())
            });
            let value = if let Some(initarg_value) = initarg_value {
                initarg_value
            } else if let Some(class_value) = &slot.class_value {
                let current = class_value.borrow().clone();
                if matches!(current, Value::Unbound) {
                    let value = slot
                        .init_form
                        .as_ref()
                        .map(|form| self.eval_in(form, environment))
                        .transpose()?
                        .unwrap_or(Value::Unbound);
                    *class_value.borrow_mut() = value.clone();
                    value
                } else {
                    current
                }
            } else {
                slot.init_form
                    .as_ref()
                    .map(|form| self.eval_in(form, environment))
                    .transpose()?
                    .unwrap_or(Value::Unbound)
            };
            slots.push((slot.name.clone(), value));
        }
        let instance = Value::instance(class.clone(), slots);
        for (initarg, value) in initargs {
            let Some(index) = class
                .slots
                .iter()
                .position(|slot| slot.initarg.as_deref() == Some(initarg.as_str()))
            else {
                return Err(Self::invalid("unknown make-instance initarg", span));
            };
            self.set_instance_slot_checked(
                &instance,
                &class.name,
                &class.slots[index].name,
                value,
                span,
            )?;
        }
        Ok(instance)
    }
}
