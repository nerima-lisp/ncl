use ncl_syntax::Form;

use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_progv(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "progv",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }

        let symbols_value = self.eval_values_in(&items[1], environment)?.primary_value();
        let symbols = symbols_value
            .list_items()
            .ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: symbols_value.type_name().to_string(),
                span: Some(items[1].span),
            })?;
        let values_value = self.eval_values_in(&items[2], environment)?.primary_value();
        let values = values_value
            .list_items()
            .ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: values_value.type_name().to_string(),
                span: Some(items[2].span),
            })?;

        let _dynamic_guard = self.dynamic_guard();
        for (index, symbol) in symbols.iter().enumerate() {
            let name = symbol.symbol_name().ok_or_else(|| {
                Self::invalid("progv symbol list must contain only symbols", items[1].span)
            })?;
            self.define_dynamic(name, values.get(index).cloned().unwrap_or(Value::Nil));
        }

        self.eval_sequence_values(&items[3..], environment)
    }
}
