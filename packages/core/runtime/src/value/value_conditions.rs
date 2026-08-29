use std::rc::Rc;

use super::{SlotValues, Value};

#[derive(Clone, Debug)]
pub struct ConditionData {
    pub(super) actual_type: String,
    pub(super) type_names: Rc<Vec<String>>,
    pub(super) slots: SlotValues,
    pub(super) message: Rc<str>,
    pub(super) format_control: Option<Rc<str>>,
    pub(super) format_arguments: Vec<Value>,
}

impl ConditionData {
    pub(super) fn equal_value(&self, other: &Self) -> bool {
        self.actual_type == other.actual_type
            && self.type_names == other.type_names
            && self.message == other.message
            && self.format_control == other.format_control
            && self.format_arguments.len() == other.format_arguments.len()
            && self
                .format_arguments
                .iter()
                .zip(other.format_arguments.iter())
                .all(|(left, right)| left.equal_value(right))
            && {
                let left_slots = self.slots.borrow();
                let right_slots = other.slots.borrow();
                left_slots.len() == right_slots.len()
                    && left_slots.iter().zip(right_slots.iter()).all(
                        |((left_name, left_value), (right_name, right_value))| {
                            left_name == right_name && left_value.equal_value(right_value)
                        },
                    )
            }
    }
}

#[derive(Clone, Debug)]
pub struct RestartData {
    pub(super) name: Rc<str>,
}
