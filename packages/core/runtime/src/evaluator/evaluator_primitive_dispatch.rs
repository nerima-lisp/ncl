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
