use super::{Environment, Runtime, RuntimeError, Span, Value};

impl Runtime {
    pub(super) fn apply_slot_reader(
        &self,
        class_name: &str,
        slot_name: &str,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("slot reader", "one", arguments.len()));
        }
        if !arguments[0].instance_is_type(class_name) {
            return Err(RuntimeError::Type {
                expected: class_name.to_string(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        }
        let Some(value) = arguments[0].instance_slot(slot_name) else {
            let function = environment
                .lookup_function("SLOT-MISSING")
                .unwrap_or_else(|| Value::primitive("SLOT-MISSING"));
            return self.apply_in(
                &function,
                &[
                    Value::class_object(
                        arguments[0]
                            .instance_class_definition()
                            .expect("validated instance has a class"),
                    ),
                    arguments[0].clone(),
                    Value::symbol(slot_name),
                    Value::symbol("SLOT-VALUE"),
                ],
                span,
                environment,
            );
        };
        if matches!(value, Value::Unbound) {
            return Err(RuntimeError::UnboundSlot {
                name: slot_name.to_owned(),
                span: Some(span),
            });
        }
        Ok(value)
    }

    pub(super) fn apply_slot_writer(
        &self,
        class_name: &str,
        slot_name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 2 {
            return Err(Self::arity("slot writer", "two", arguments.len()));
        }
        let value = arguments[0].clone();
        let object = &arguments[1];
        if !object.instance_is_type(class_name) {
            return Err(RuntimeError::Type {
                expected: class_name.to_string(),
                actual: object.type_name().to_string(),
                span: Some(span),
            });
        }
        self.set_instance_slot_checked(object, class_name, slot_name, value.clone(), span)?;
        Ok(value)
    }

    pub(super) fn apply_condition_reader(
        condition_name: &str,
        slot_name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("condition reader", "one", arguments.len()));
        }
        arguments[0]
            .condition_slot(condition_name, slot_name)
            .ok_or_else(|| Self::invalid("condition slot is not defined", span))
    }

    pub(super) fn apply_condition_writer(
        condition_name: &str,
        slot_name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 2 {
            return Err(Self::arity("condition writer", "two", arguments.len()));
        }
        let value = arguments[0].clone();
        let object = &arguments[1];
        if object.set_condition_slot(condition_name, slot_name, value.clone()) {
            Ok(value)
        } else {
            Err(Self::invalid("condition slot is not defined", span))
        }
    }
}
