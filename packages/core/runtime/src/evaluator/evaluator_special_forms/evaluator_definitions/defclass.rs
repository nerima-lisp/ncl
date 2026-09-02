#![allow(clippy::wildcard_imports)]
use super::*;

mod options;
mod options_tests;
mod slots;
mod slots_tests;

impl Runtime {
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
            direct_superclasses: direct_superclasses.into_iter().map(Into::into).collect(),
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
}
