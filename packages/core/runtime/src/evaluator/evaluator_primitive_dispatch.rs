#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if let Some(result) = self.apply_sequence_primitive(name, arguments, environment, span) {
            return result;
        }
        if let Some(result) =
            self.apply_symbol_property_primitive(name, arguments, environment, span)
        {
            return result;
        }
        if let Some(result) =
            Self::apply_class_introspection_primitive(name, arguments, environment, span)
        {
            return result;
        }
        if let Some(result) = self.apply_symbol_creation_primitive(name, arguments, span) {
            return result;
        }
        if let Some(result) = self.apply_package_introspection_primitive(name, arguments, span) {
            return result;
        }
        if let Some(result) = self.apply_package_creation_primitive(name, arguments, span) {
            return result;
        }
        if let Some(result) = self.apply_symbol_value_primitive(name, arguments, environment, span)
        {
            return result;
        }
        if let Some(result) =
            self.apply_symbol_function_primitive(name, arguments, environment, span)
        {
            return result;
        }
        if let Some(result) = Self::apply_slot_primitive(name, arguments, span) {
            return match result {
                Err(RuntimeError::InvalidForm { ref message, .. })
                    if message == "slot is not defined for this class"
                        && matches!(name, "SLOT-VALUE" | "SLOT-VALUE-USING-CLASS") =>
                {
                    let (object, slot_name) = if name.ends_with("-USING-CLASS") {
                        (&arguments[1], &arguments[2])
                    } else {
                        (&arguments[0], &arguments[1])
                    };
                    let class = object
                        .instance_class_definition()
                        .ok_or_else(|| Self::invalid("object has no class definition", span))?;
                    let function = environment
                        .lookup_function("SLOT-MISSING")
                        .unwrap_or_else(|| Value::primitive("SLOT-MISSING"));
                    self.apply_in(
                        &function,
                        &[
                            Value::class_object(class),
                            object.clone(),
                            slot_name.clone(),
                            Value::symbol("SLOT-VALUE"),
                        ],
                        span,
                        environment,
                    )
                }
                Err(RuntimeError::UnboundSlot { .. })
                    if matches!(name, "SLOT-VALUE" | "SLOT-VALUE-USING-CLASS") =>
                {
                    let (object, slot_name) = if name.ends_with("-USING-CLASS") {
                        (&arguments[1], &arguments[2])
                    } else {
                        (&arguments[0], &arguments[1])
                    };
                    let class = object
                        .instance_class_definition()
                        .ok_or_else(|| Self::invalid("object has no class definition", span))?;
                    let function = environment
                        .lookup_function("SLOT-UNBOUND")
                        .unwrap_or_else(|| Value::primitive("SLOT-UNBOUND"));
                    self.apply_in(
                        &function,
                        &[Value::class_object(class), object.clone(), slot_name.clone()],
                        span,
                        environment,
                    )
                }
                other => other,
            };
        }
        if let Some(result) = Self::apply_slot_definition_primitive(name, arguments, span) {
            return result;
        }
        if let Some(result) = self.apply_restart_primitive(name, arguments, environment, span) {
            return result;
        }
        if let Some(result) = self.apply_package_use_primitive(name, arguments, span) {
            return result;
        }
        if let Some(result) = self.apply_package_symbol_primitive(name, arguments, span) {
            return result;
        }
        if let Some(result) = self.apply_method_primitive(name, arguments, environment, span) {
            return result;
        }
        if let Some(result) = self.apply_package_listing_primitive(name, arguments, span) {
            return result;
        }
        if let Some(result) = self.apply_evaluation_primitive(name, arguments, environment, span) {
            return result;
        }
        if let Some(result) = self.apply_condition_primitive(name, arguments, environment, span) {
            return result;
        }
        match name {
            "MAP" | "REDUCE" | "MERGE" | "MAP-INTO" => {
                self.apply_sequence_collection(name, arguments, environment, span)
            }
            _ => Err(Self::invalid("unknown runtime primitive", span)),
        }
    }
}
