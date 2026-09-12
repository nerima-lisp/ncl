use std::collections::HashSet;

use ncl_syntax::{Form, Span};

use crate::error::{SignaledError, normalize_condition_name};
use crate::{Environment, ReturnValue, Runtime, RuntimeError, Value, builtins};

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
        condition_types: &[String],
        message: String,
        format_control: Option<String>,
        format_arguments: &[Value],
        warning: bool,
        span: Span,
    ) -> RuntimeError {
        RuntimeError::Signaled(Box::new(SignaledError {
            condition: normalize_condition_name(condition).into(),
            condition_types: condition_types
                .iter()
                .map(|name| normalize_condition_name(name).into())
                .collect(),
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
        Self::new().make_condition_in(arguments, &Environment::new(), span)
    }

    pub(crate) fn make_condition_in(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(Self::arity(
                "make-condition",
                "at least one",
                arguments.len(),
            ));
        }
        let initarg_values = &arguments[1..];
        if !initarg_values.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "make-condition initargs must be keyword/value pairs",
                span,
            ));
        }

        let actual_type = Self::name_designator_from_value(&arguments[0], span)?;
        let mut type_names = Vec::new();
        let mut condition_initargs = Vec::new();
        let mut initforms = Vec::new();
        let mut visiting = HashSet::new();
        Self::condition_metadata(
            &actual_type,
            environment,
            &mut type_names,
            &mut condition_initargs,
            &mut initforms,
            &mut visiting,
        );
        let mut format_control = None;
        let mut format_arguments = Vec::new();
        let mut slots = Vec::new();
        for pair in initarg_values.as_chunks::<2>().0 {
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
                "NAME" => slots.push(("NAME".to_owned(), pair[1].clone())),
                _ => match condition_initargs.iter().find(|(name, _)| name == &initarg) {
                    Some((_, slot_name)) => {
                        Self::set_condition_slot_value(&mut slots, slot_name, pair[1].clone())
                    }
                    None => {
                        return Err(Self::invalid(
                            &format!("unknown make-condition initarg :{initarg}"),
                            span,
                        ));
                    }
                },
            }
        }
        for (slot_name, form) in initforms {
            if !slots
                .iter()
                .any(|(name, _): &(String, Value)| name == &slot_name)
            {
                Self::set_condition_slot_value(
                    &mut slots,
                    &slot_name,
                    self.eval_in(&form, environment)?,
                );
            }
        }

        let message = match format_control.as_deref() {
            Some(control) => builtins::format_control(control, &format_arguments)?,
            None => String::new(),
        };
        Ok(Value::condition_from_parts_with_types(
            actual_type,
            type_names,
            slots,
            message,
            format_control,
            format_arguments,
        ))
    }

    fn set_condition_slot_value(slots: &mut Vec<(String, Value)>, name: &str, value: Value) {
        if let Some((_, current)) = slots.iter_mut().find(|(slot_name, _)| slot_name == name) {
            *current = value;
        } else {
            slots.push((name.to_owned(), value));
        }
    }

    fn condition_metadata(
        name: &str,
        environment: &Environment,
        type_names: &mut Vec<String>,
        initargs: &mut Vec<(String, String)>,
        initforms: &mut Vec<(String, Form)>,
        visiting: &mut HashSet<String>,
    ) {
        if !visiting.insert(name.to_owned()) {
            return;
        }
        type_names.push(name.to_owned());
        for (initarg, slot_name) in match name {
            "ARITHMETIC-ERROR" => [("OPERATION", "OPERATION"), ("OPERANDS", "OPERANDS")].as_slice(),
            "FILE-ERROR" => [("PATHNAME", "PATHNAME")].as_slice(),
            "PACKAGE-ERROR" => [("PACKAGE", "PACKAGE")].as_slice(),
            "STREAM-ERROR" => [("STREAM", "STREAM")].as_slice(),
            "TYPE-ERROR" => [("DATUM", "DATUM"), ("EXPECTED-TYPE", "EXPECTED-TYPE")].as_slice(),
            "UNBOUND-SLOT" => [("INSTANCE", "INSTANCE")].as_slice(),
            _ => &[],
        } {
            if !initargs.iter().any(|(name, _)| name == initarg) {
                initargs.push((initarg.to_string(), slot_name.to_string()));
            }
        }
        if let Some(definition) = environment.lookup_condition(name) {
            for parent in definition.parents {
                Self::condition_metadata(
                    &parent,
                    environment,
                    type_names,
                    initargs,
                    initforms,
                    visiting,
                );
            }
            for (initarg, slot_name) in definition.initargs {
                if let Some(existing) = initargs.iter_mut().find(|(name, _)| name == &initarg) {
                    existing.1 = slot_name;
                } else {
                    initargs.push((initarg, slot_name));
                }
            }
            for (slot_name, form) in definition.initforms {
                if let Some(existing) = initforms.iter_mut().find(|(name, _)| name == &slot_name) {
                    existing.1 = form;
                } else {
                    initforms.push((slot_name, form));
                }
            }
        }
    }
}
