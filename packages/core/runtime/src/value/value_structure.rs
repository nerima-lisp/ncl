use std::cell::RefCell;
use std::rc::Rc;

use super::Value;

impl Value {
    pub(crate) fn structure_with_types(
        name: impl AsRef<str>,
        slots: Vec<(String, Self)>,
        mut type_names: Vec<String>,
    ) -> Self {
        let name = name.as_ref().to_string();
        if !type_names
            .iter()
            .any(|type_name| type_name.eq_ignore_ascii_case(&name))
        {
            type_names.insert(0, name.clone());
        }
        Self::Structure {
            name: Rc::from(name),
            types: Rc::new(type_names.into_iter().map(Rc::<str>::from).collect()),
            slots: Rc::new(RefCell::new(
                slots
                    .into_iter()
                    .map(|(slot_name, value)| (Rc::from(slot_name), value))
                    .collect(),
            )),
        }
    }

    pub(crate) fn structure_name(&self) -> Option<&str> {
        match self {
            Self::Structure { name, .. } => Some(name),
            _ => None,
        }
    }

    pub(crate) fn structure_is_type(&self, expected: &str) -> bool {
        match self {
            Self::Structure { types, .. } => types
                .iter()
                .any(|type_name| type_name.eq_ignore_ascii_case(expected)),
            _ => false,
        }
    }

    pub(crate) fn structure_slot(&self, index: usize) -> Option<Self> {
        match self {
            Self::Structure { slots, .. } => {
                slots.borrow().get(index).map(|(_, value)| value.clone())
            }
            _ => None,
        }
    }

    pub(crate) fn set_structure_slot(
        &self,
        structure_name: &str,
        index: usize,
        value: Self,
    ) -> bool {
        let Self::Structure { slots, .. } = self else {
            return false;
        };
        if !self.structure_is_type(structure_name) {
            return false;
        }
        let mut slots = slots.borrow_mut();
        let Some((_, slot_value)) = slots.get_mut(index) else {
            return false;
        };
        *slot_value = value;
        true
    }

    pub(crate) fn copy_structure(&self) -> Option<Self> {
        let Self::Structure { name, types, slots } = self else {
            return None;
        };
        Some(Self::Structure {
            name: name.clone(),
            types: types.clone(),
            slots: Rc::new(RefCell::new(slots.borrow().clone())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Value;

    #[test]
    fn structure_accessors_reject_non_structure_and_out_of_range_targets() {
        assert!(Value::Nil.structure_slot(0).is_none());
        assert!(!Value::Nil.set_structure_slot("point", 0, Value::Integer(1)));
        assert!(Value::Nil.copy_structure().is_none());

        let point = Value::structure_with_types(
            "point",
            vec![("x".to_owned(), Value::Integer(1))],
            Vec::new(),
        );
        assert!(!point.set_structure_slot("circle", 0, Value::Integer(2)));
        assert!(!point.set_structure_slot("point", 5, Value::Integer(2)));
        assert!(point.set_structure_slot("point", 0, Value::Integer(9)));
        assert!(
            point
                .structure_slot(0)
                .is_some_and(|value| value.equal_value(&Value::Integer(9)))
        );
    }
}
