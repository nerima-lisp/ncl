#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_evaluation_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(
            name,
            "EVAL"
                | "COMPILE"
                | "LOAD"
                | "MAKE-INSTANCE"
                | "ALLOCATE-INSTANCE"
                | "INITIALIZE-INSTANCE"
                | "SHARED-INITIALIZE"
                | "REINITIALIZE-INSTANCE"
                | "PROVIDE"
                | "REQUIRE"
        ) {
            return None;
        }
        let result = match name {
            "EVAL" => match arguments.len() {
                1 => Self::form_from_value(&arguments[0], span)
                    .and_then(|form| self.eval_values_in(&form, environment)),
                _ => Err(Self::arity("eval", "one", arguments.len())),
            },
            "COMPILE" => self.compile_function(arguments, environment, span),
            "LOAD" => self.load_file(arguments, span),
            "MAKE-INSTANCE" => self.make_instance(arguments, environment, span),
            "ALLOCATE-INSTANCE" => self.allocate_instance(arguments, environment, span),
            "INITIALIZE-INSTANCE" => self.initialize_instance(arguments, environment, span),
            "SHARED-INITIALIZE" => self.shared_initialize(arguments, environment, span),
            "REINITIALIZE-INSTANCE" => self.reinitialize_instance(arguments, environment, span),
            "PROVIDE" => self.provide_feature(arguments, span),
            "REQUIRE" => self.require_feature(arguments, span),
            _ => unreachable!("evaluation primitive name was prevalidated"),
        };
        Some(result)
    }

    fn provide_feature(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("provide", "one", arguments.len()));
        }
        let feature = arguments[0].clone();
        let name = Self::name_designator_from_value(&feature, span)?.to_string();
        let features = self
            .lookup_special("*FEATURES*")
            .and_then(|value| value.list_items())
            .ok_or_else(|| Self::invalid("*FEATURES* must be a list", span))?;
        if !features.iter().any(|item| {
            Self::name_designator_from_value(item, span)
                .is_ok_and(|item_name| item_name.eq_ignore_ascii_case(&name))
        }) {
            let mut updated = features;
            updated.push(feature);
            self.define_special_value("*FEATURES*", Value::list(updated), true);
        }
        Ok(Value::boolean(true))
    }

    fn require_feature(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if !(1..=2).contains(&arguments.len()) {
            return Err(Self::arity("require", "one or two", arguments.len()));
        }
        let feature = &arguments[0];
        let name = Self::name_designator_from_value(feature, span)?.to_string();
        let is_present = || {
            self.lookup_special("*FEATURES*")
                .and_then(|value| value.list_items())
                .is_some_and(|features| {
                    features.iter().any(|item| {
                        Self::name_designator_from_value(item, span)
                            .is_ok_and(|item_name| item_name.eq_ignore_ascii_case(&name))
                    })
                })
        };
        if is_present() {
            return Ok(Value::boolean(true));
        }
        if let Some(paths) = arguments.get(1) {
            let paths = paths.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_owned(),
                actual: paths.type_name().to_owned(),
                span: Some(span),
            })?;
            for path in paths {
                self.load_file(&[path], span)?;
            }
        }
        if is_present() {
            Ok(Value::boolean(true))
        } else {
            Err(Self::invalid(
                &format!("required feature {name} was not provided"),
                span,
            ))
        }
    }
}
