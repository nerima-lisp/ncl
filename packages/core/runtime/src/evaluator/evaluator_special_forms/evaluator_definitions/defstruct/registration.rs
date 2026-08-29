use super::{
    Environment, OrdinaryLambdaList, Rc, Runtime, StructureDefinition, StructureSlot, Value,
};

pub(super) struct DefstructRegistration {
    pub(super) structure_name: String,
    pub(super) structure_types: Vec<String>,
    pub(super) slots: Vec<StructureSlot>,
    pub(super) constructor_options: Vec<(Option<String>, Option<OrdinaryLambdaList>)>,
    pub(super) predicate_name: Option<String>,
    pub(super) copier_name: Option<String>,
    pub(super) conc_name: String,
}

impl Runtime {
    pub(super) fn register_defstruct(
        environment: &Environment,
        registration: DefstructRegistration,
    ) {
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
}
