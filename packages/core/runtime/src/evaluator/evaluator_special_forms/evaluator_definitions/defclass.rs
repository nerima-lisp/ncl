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
        if direct_superclasses.is_empty() && !class_name.eq_ignore_ascii_case("STANDARD-OBJECT") {
            direct_superclasses.push("STANDARD-OBJECT".to_owned());
        }

        let slot_forms = Self::list_form_items(&items[3], "defclass slot list")?;
        let mut slots: Vec<ClassSlot> = Vec::new();
        let mut readers = Vec::new();
        let mut writers = Vec::new();
        let mut setf_writers = Vec::new();
        let mut default_initargs = Vec::new();
        let mut documentation = None;

        for slot_form in slot_forms {
            let registration = Self::parse_defclass_slot(slot_form)?;
            let slot_name = registration.slot.name.clone();
            readers.extend(registration.readers);
            writers.extend(registration.writers);
            setf_writers.extend(registration.setf_writers);
            let mut slot = registration.slot;
            slot.init_function = slot
                .init_form
                .as_ref()
                .map(|form| Value::closure(Vec::new(), vec![form.clone()], environment.clone()));
            if let Some(existing) = slots.iter_mut().find(|slot| slot.name == slot_name) {
                *existing = slot;
            } else {
                slots.push(slot);
            }
        }

        for option in items.iter().skip(4) {
            Self::parse_defclass_option(option, &mut default_initargs, &mut documentation)?;
        }
        let direct_default_initargs = default_initargs.clone();

        let direct_slot_count = slots.len();
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
            documentation,
            direct_superclasses: direct_superclasses.into_iter().map(Into::into).collect(),
            direct_slots: slots
                .iter()
                .take(direct_slot_count)
                .map(|slot| slot.name.clone().into())
                .collect(),
            direct_default_initargs,
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
        for (accessor_name, slot_name) in setf_writers {
            environment.define_setf_function(
                &accessor_name,
                Value::slot_writer(class_name.clone(), slot_name),
            );
        }
        Ok(Value::symbol(class_name))
    }
}
