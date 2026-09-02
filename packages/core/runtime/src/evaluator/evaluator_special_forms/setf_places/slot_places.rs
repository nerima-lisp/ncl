use super::{Environment, Form, Runtime, RuntimeError, Span, Value};

impl Runtime {
    pub(super) fn set_slot_value_place(
        &self,
        args: &[Form],
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if args.len() != 2 {
            return Err(Self::arity("setf slot-value", "two", args.len()));
        }
        let current = self.eval_in(&args[0], environment)?;
        let slot = self.eval_in(&args[1], environment)?;
        let slot_name = Self::slot_name_from_value(&slot, span)?;
        let Some(class) = current.instance_class_definition() else {
            return Err(RuntimeError::Type {
                expected: "STANDARD-OBJECT".to_string(),
                actual: current.type_name().to_string(),
                span: Some(args[0].span),
            });
        };
        self.set_instance_slot_checked(&current, &class.name, &slot_name, value, span)
    }

    pub(super) fn set_function_place(
        &self,
        function: &crate::Function,
        args: &[Form],
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<Option<()>, RuntimeError> {
        match function {
            crate::Function::SlotReader {
                class_name,
                slot_name,
            } => {
                if args.len() != 1 {
                    return Err(Self::arity("setf slot accessor", "one", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                if !current.instance_is_type(class_name) {
                    return Err(RuntimeError::Type {
                        expected: class_name.clone(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                }
                self.set_instance_slot_checked(&current, class_name, slot_name, value, span)?;
                Ok(Some(()))
            }
            crate::Function::StructureAccessor {
                structure_name,
                slot_index,
                read_only,
                ..
            } => {
                if args.len() != 1 {
                    return Err(Self::arity("setf structure accessor", "one", args.len()));
                }
                if *read_only {
                    return Err(Self::invalid(
                        "cannot SETF a read-only structure slot",
                        span,
                    ));
                }
                let current = self.eval_in(&args[0], environment)?;
                if current.set_structure_slot(structure_name, *slot_index, value) {
                    Ok(Some(()))
                } else {
                    Err(RuntimeError::Type {
                        expected: structure_name.clone(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    })
                }
            }
            _ => Ok(None),
        }
    }
}
