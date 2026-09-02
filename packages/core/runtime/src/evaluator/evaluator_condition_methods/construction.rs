use ncl_syntax::Span;

use crate::error::{ConditionName, SignaledError, normalize_condition_name};
use crate::{Environment, ReturnValue, Runtime, RuntimeError, Value, builtins};

impl Runtime {
    pub(crate) fn make_condition_in(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Self::make_condition(arguments, span);
        }
        let actual_type = Self::name_designator_from_value(&arguments[0], span)?;
        if environment.lookup_condition(&actual_type).is_none() {
            return Self::make_condition(arguments, span);
        }
        let initargs = &arguments[1..];
        if !initargs.len().is_multiple_of(2) {
            return Err(Self::invalid("make-condition initargs must be keyword/value pairs", span));
        }
        fn find_condition_initarg(
            environment: &Environment,
            name: &str,
            initarg: &str,
            visited: &mut std::collections::HashSet<String>,
        ) -> Option<String> {
            if !visited.insert(name.to_owned()) {
                return None;
            }
            let definition = environment.lookup_condition(name)?;
            if let Some((_, slot_name)) = definition
                .initargs
                .iter()
                .find(|(name, _)| name == initarg)
            {
                return Some(slot_name.clone());
            }
            definition.parents.iter().find_map(|parent| {
                find_condition_initarg(environment, parent, initarg, visited)
            })
        }
        let mut slots = Vec::new();
        for pair in initargs.as_chunks::<2>().0 {
            let initarg = Self::name_designator_from_value(&pair[0], span)?;
            let Some(slot_name) = find_condition_initarg(
                environment,
                &actual_type,
                &initarg,
                &mut std::collections::HashSet::new(),
            ) else {
                return Err(Self::invalid(&format!("unknown make-condition initarg :{initarg}"), span));
            };
            slots.push((slot_name.clone(), pair[1].clone()));
        }
        fn append_condition_initforms(
            runtime: &Runtime,
            environment: &Environment,
            name: &str,
            slots: &mut Vec<(String, Value)>,
            explicit: &std::collections::HashSet<String>,
            visited: &mut std::collections::HashSet<String>,
        ) -> Result<(), RuntimeError> {
            if !visited.insert(name.to_owned()) {
                return Ok(());
            }
            let Some(definition) = environment.lookup_condition(name) else {
                return Ok(());
            };
            for parent in &definition.parents {
                append_condition_initforms(runtime, environment, parent, slots, explicit, visited)?;
            }
            for (slot_name, form) in &definition.initforms {
                if !explicit.contains(slot_name)
                    && !slots.iter().any(|(name, _)| name.eq_ignore_ascii_case(slot_name))
                {
                    slots.push((slot_name.clone(), runtime.eval_values_in(form, environment)?.primary_value()));
                }
            }
            Ok(())
        }
        let explicit = slots.iter().map(|(name, _)| name.clone()).collect();
        append_condition_initforms(
            self,
            environment,
            &actual_type,
            &mut slots,
            &explicit,
            &mut std::collections::HashSet::new(),
        )?;
        fn append_condition_types(
            environment: &Environment,
            name: &str,
            type_names: &mut Vec<String>,
            visited: &mut std::collections::HashSet<String>,
        ) {
            if !visited.insert(name.to_owned()) {
                return;
            }
            type_names.push(name.to_owned());
            if let Some(definition) = environment.lookup_condition(name) {
                for parent in definition.parents {
                    append_condition_types(environment, &parent, type_names, visited);
                }
            }
        }

        let mut type_names = Vec::new();
        append_condition_types(
            environment,
            &actual_type,
            &mut type_names,
            &mut std::collections::HashSet::new(),
        );
        Ok(Value::condition_from_parts_with_types(
            actual_type,
            type_names,
            slots,
            String::new(),
            None,
            Vec::new(),
        ))
    }

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
                "DATUM" | "EXPECTED-TYPE" | "NAME" | "OPERATION" | "OPERANDS" | "PACKAGE"
                | "PATHNAME" | "STREAM" | "INSTANCE" => {
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
