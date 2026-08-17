impl Runtime {
    fn set_accessor_place(
        &self,
        function: &crate::Function,
        args: &[Form],
        value: &Value,
        place: &Form,
        environment: &Environment,
    ) -> Option<Result<(), RuntimeError>> {
        Some(match function {
            crate::Function::SlotReader {
                class_name,
                slot_name,
            } => {
                if args.len() != 1 {
                    return Some(Err(self.arity("setf slot accessor", "one", args.len())));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                if !current.instance_is_type(class_name) {
                    return Some(Err(RuntimeError::Type {
                        expected: class_name.clone(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    }));
                }
                if current.set_instance_slot(class_name, slot_name, value.clone()) {
                    Ok(())
                } else {
                    Err(self.invalid("slot is not defined for this class", place.span))
                }
            }
            crate::Function::ConditionReader {
                condition_name,
                slot_name,
            } => {
                if args.len() != 1 {
                    return Some(Err(self.arity(
                        "setf condition accessor",
                        "one",
                        args.len(),
                    )));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                if current.set_condition_slot(condition_name, slot_name, value.clone()) {
                    Ok(())
                } else {
                    Err(RuntimeError::Type {
                        expected: condition_name.clone(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    })
                }
            }
            crate::Function::StructureAccessor {
                structure_name,
                slot_index,
                read_only,
                ..
            } => {
                if args.len() != 1 {
                    return Some(Err(self.arity(
                        "setf structure accessor",
                        "one",
                        args.len(),
                    )));
                }
                if *read_only {
                    return Some(Err(
                        self.invalid("cannot SETF a read-only structure slot", place.span),
                    ));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                if current.set_structure_slot(structure_name, *slot_index, value.clone()) {
                    Ok(())
                } else {
                    Err(RuntimeError::Type {
                        expected: structure_name.clone(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    })
                }
            }
            _ => return None,
        })
    }
}
