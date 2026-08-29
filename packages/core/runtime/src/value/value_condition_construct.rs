use std::cell::RefCell;
use std::rc::Rc;

use crate::error::{ReturnValue, RuntimeError};

use super::{ConditionData, RestartData, Value};

impl Value {
    pub(crate) fn condition(error: &RuntimeError) -> Self {
        let (actual_type, type_names, message, format_control, format_arguments) = match error {
            RuntimeError::Signaled(error) => (
                if error.warning {
                    "SIMPLE-WARNING".to_owned()
                } else {
                    error.condition.clone()
                },
                if error.condition_types.is_empty() {
                    vec![error.condition.clone()]
                } else {
                    error.condition_types.to_vec()
                },
                error.message.clone(),
                error.format_control.clone(),
                error
                    .format_arguments
                    .iter()
                    .cloned()
                    .map(ReturnValue::into_value)
                    .collect(),
            ),
            _ => (
                error.condition_type_name(),
                vec![error.condition_type_name()],
                error.to_string(),
                None,
                Vec::new(),
            ),
        };
        Self::condition_from_parts_with_types(
            actual_type,
            type_names,
            Vec::new(),
            message,
            format_control,
            format_arguments,
        )
    }

    pub(crate) fn condition_from_parts(
        actual_type: String,
        message: String,
        format_control: Option<String>,
        format_arguments: Vec<Self>,
    ) -> Self {
        Self::condition_from_parts_with_types(
            actual_type.clone(),
            vec![actual_type],
            Vec::new(),
            message,
            format_control,
            format_arguments,
        )
    }

    pub(super) fn condition_from_parts_with_types(
        actual_type: String,
        type_names: Vec<String>,
        slots: Vec<(String, Self)>,
        message: String,
        format_control: Option<String>,
        format_arguments: Vec<Self>,
    ) -> Self {
        Self::Condition(Rc::new(ConditionData {
            actual_type,
            type_names: Rc::new(type_names),
            slots: Rc::new(RefCell::new(
                slots
                    .into_iter()
                    .map(|(name, value)| (Rc::from(name.as_str()), value))
                    .collect(),
            )),
            message: Rc::from(message),
            format_control: format_control.map(|value| Rc::from(value.as_str())),
            format_arguments,
        }))
    }

    pub(crate) fn restart(name: impl AsRef<str>) -> Self {
        Self::Restart(Rc::new(RestartData {
            name: Rc::from(name.as_ref()),
        }))
    }
}
