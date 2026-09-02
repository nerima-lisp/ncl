use std::cell::RefCell;
use std::rc::Rc;

use super::{ClassDefinition, Instance, Value};

impl Value {
    pub(crate) const fn class_object(definition: Rc<ClassDefinition>) -> Self {
        Self::Class(definition)
    }

    pub(crate) fn instance(definition: Rc<ClassDefinition>, slots: Vec<(String, Self)>) -> Self {
        Self::Instance(Instance {
            class: definition,
            slots: Rc::new(RefCell::new(
                slots
                    .into_iter()
                    .map(|(slot_name, value)| (Rc::from(slot_name), value))
                    .collect(),
            )),
        })
    }

    pub(crate) fn instance_class_definition(&self) -> Option<Rc<ClassDefinition>> {
        match self {
            Self::Instance(instance) => Some(instance.class.clone()),
            _ => None,
        }
    }

    pub(crate) fn instance_is_type(&self, expected: &str) -> bool {
        let Self::Instance(instance) = self else {
            return false;
        };
        instance
            .class
            .precedence
            .iter()
            .any(|class_name| class_name.eq_ignore_ascii_case(expected))
    }

    pub(crate) fn instance_slot(&self, slot_name: &str) -> Option<Self> {
        let Self::Instance(instance) = self else {
            return None;
        };
        if let Some(slot) = instance
            .class
            .slots
            .iter()
            .find(|slot| slot.name.eq_ignore_ascii_case(slot_name))
            && let Some(class_value) = &slot.class_value
        {
            return Some(class_value.borrow().clone());
        }
        instance
            .slots
            .borrow()
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(slot_name))
            .map(|(_, value)| value.clone())
    }

    pub(crate) fn instance_slot_exists(&self, slot_name: &str) -> bool {
        let Self::Instance(instance) = self else {
            return false;
        };
        if instance
            .class
            .slots
            .iter()
            .any(|slot| slot.name.eq_ignore_ascii_case(slot_name) && slot.class_value.is_some())
        {
            return true;
        }
        instance
            .slots
            .borrow()
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(slot_name))
    }

    pub(crate) fn instance_slot_is_bound(&self, slot_name: &str) -> Option<bool> {
        self.instance_slot(slot_name)
            .map(|value| !matches!(value, Self::Unbound))
    }

    pub(crate) fn set_instance_slot(&self, class_name: &str, slot_name: &str, value: Self) -> bool {
        let Self::Instance(instance) = self else {
            return false;
        };
        if !self.instance_is_type(class_name) {
            return false;
        }
        if let Some(slot) = instance
            .class
            .slots
            .iter()
            .find(|slot| slot.name.eq_ignore_ascii_case(slot_name))
            && let Some(class_value) = &slot.class_value
        {
            *class_value.borrow_mut() = value;
            return true;
        }
        let mut slots = instance.slots.borrow_mut();
        let Some((_, slot_value)) = slots
            .iter_mut()
            .find(|(name, _)| name.eq_ignore_ascii_case(slot_name))
        else {
            return false;
        };
        *slot_value = value;
        true
    }
}
