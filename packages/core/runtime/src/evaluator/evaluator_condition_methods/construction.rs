use ncl_syntax::Span;

use crate::error::{ConditionName, SignaledError, normalize_condition_name};
use crate::{ReturnValue, Runtime, RuntimeError, Value, builtins};

impl Runtime {
    pub(crate) fn condition_format_control(value: &Value) -> Option<String> {
        match value {
            Value::String(control) => Some(control.to_string()),
            _ => None,
        }
    }

    pub(crate) fn condition_message(
        value: &Value,
        arguments: &[Value],
        span: Span,
    ) -> Result<String, RuntimeError> {
        match value {
            Value::String(control) => builtins::format_control(control, arguments),
            value if arguments.is_empty() => Ok(value.to_string()),
            value => Err(RuntimeError::Type {
                expected: "a string format control".to_owned(),
                actual: value.type_name().to_owned(),
                span: Some(span),
            }),
        }
    }

    pub(crate) fn signaled_error(
        condition: &str,
        condition_types: &[ConditionName],
        message: String,
        format_control: Option<String>,
        format_arguments: &[Value],
        warning: bool,
        span: Span,
    ) -> RuntimeError {
        RuntimeError::Signaled(Box::new(SignaledError {
            condition: normalize_condition_name(condition),
            condition_types: condition_types.to_vec().into_boxed_slice(),
            message,
            format_control,
            format_arguments: format_arguments
                .iter()
                .cloned()
                .map(ReturnValue::new)
                .collect(),
            warning,
            span: Some(span),
        }))
    }

    pub(crate) fn condition_error(
        value: &Value,
        warning: bool,
        span: Span,
    ) -> Result<RuntimeError, RuntimeError> {
        let Some(condition) = value.condition_type_name() else {
            return Err(RuntimeError::Type {
                expected: "CONDITION".to_owned(),
                actual: value.type_name().to_owned(),
                span: Some(span),
            });
        };
        let message = value.condition_message().unwrap_or_default().to_owned();
        let format_control = value
            .simple_condition_format_control()
            .map(ToOwned::to_owned);
        let format_arguments = value
            .simple_condition_format_arguments()
            .unwrap_or_default();
        Ok(Self::signaled_error(
            condition,
            &value.condition_type_names().unwrap_or_default(),
            message,
            format_control,
            &format_arguments,
            warning,
            span,
        ))
    }

    pub(crate) fn make_condition(arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(Self::arity(
                "make-condition",
                "at least one",
                arguments.len(),
            ));
        }
        let initargs = &arguments[1..];
        if !initargs.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "make-condition initargs must be keyword/value pairs",
                span,
            ));
        }

        let actual_type = Self::name_designator_from_value(&arguments[0], span)?;
        let mut format_control = None;
        let mut format_arguments = Vec::new();
        let mut slots = Vec::new();
        for pair in initargs.as_chunks::<2>().0 {
            let initarg = Self::name_designator_from_value(&pair[0], span)?;
            match initarg.as_str() {
                "FORMAT-CONTROL" => {
                    let Value::String(control) = &pair[1] else {
                        return Err(RuntimeError::Type {
                            expected: "STRING".to_owned(),
                            actual: pair[1].type_name().to_owned(),
                            span: Some(span),
                        });
                    };
                    format_control = Some(control.to_string());
                }
                "FORMAT-ARGUMENTS" => {
                    format_arguments = pair[1].list_items().ok_or_else(|| RuntimeError::Type {
                        expected: "PROPER-LIST".to_owned(),
                        actual: pair[1].type_name().to_owned(),
                        span: Some(span),
                    })?;
                }
                "DATUM" | "EXPECTED-TYPE" => {
                    slots.push((initarg, pair[1].clone()));
                }
                _ => {
                    return Err(Self::invalid(
                        &format!("unknown make-condition initarg :{initarg}"),
                        span,
                    ));
                }
            }
        }

        let message = match format_control.as_deref() {
            Some(control) => builtins::format_control(control, &format_arguments)?,
            None => String::new(),
        };
        Ok(Value::condition_from_parts_with_types(
            actual_type.clone(),
            vec![actual_type],
            slots,
            message,
            format_control,
            format_arguments,
        ))
    }
}
