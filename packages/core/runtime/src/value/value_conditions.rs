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

#[derive(Clone, Debug)]
pub struct RestartData {
    pub(super) name: Rc<str>,
}
