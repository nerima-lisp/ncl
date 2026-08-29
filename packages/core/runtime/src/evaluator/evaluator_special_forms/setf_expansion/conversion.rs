use super::{Form, Runtime, RuntimeError, SetfExpansion, Span, Value};

impl Runtime {
    fn setf_expansion_forms(
        value: &Value,
        label: &str,
        span: Span,
    ) -> Result<Vec<Form>, RuntimeError> {
        let Some(values) = value.list_items() else {
            return Err(Self::invalid(
                &format!("SETF expansion {label} must be a proper list"),
                span,
            ));
        };
        values
            .iter()
            .map(|value| Self::form_from_value(value, span))
            .collect()
    }

    pub(super) fn parse_setf_expansion(
        value: &Value,
        span: Span,
    ) -> Result<SetfExpansion, RuntimeError> {
        let values = value.multiple_values();
        if values.len() != 5 {
            return Err(Self::invalid("SETF expander must return five values", span));
        }
        let temporaries = Self::setf_expansion_forms(&values[0], "temporary variables", span)?;
        let value_forms = Self::setf_expansion_forms(&values[1], "value forms", span)?;
        if temporaries.len() != value_forms.len() {
            return Err(Self::invalid(
                "SETF expansion temporary and value lists must have the same length",
                span,
            ));
        }
        let mut stores = Self::setf_expansion_forms(&values[2], "store variables", span)?;
        if stores.len() != 1 {
            return Err(Self::invalid(
                "SETF expansion must provide exactly one store variable",
                span,
            ));
        }
        Ok(SetfExpansion {
            temporaries,
            values: value_forms,
            store: stores.remove(0),
            store_form: Self::form_from_value(&values[3], span)?,
            access_form: Self::form_from_value(&values[4], span)?,
        })
    }

    pub(in crate::evaluator::evaluator_special_forms) fn setf_expansion_value(
        expansion: &SetfExpansion,
        _span: Span,
    ) -> Result<Value, RuntimeError> {
        let list_value = |forms: &[Form]| {
            forms
                .iter()
                .map(Self::quoted_value)
                .collect::<Result<Vec<_>, _>>()
                .map(Value::list)
        };
        Ok(Value::values(vec![
            list_value(&expansion.temporaries)?,
            list_value(&expansion.values)?,
            Value::list(vec![Self::quoted_value(&expansion.store)?]),
            Self::quoted_value(&expansion.store_form)?,
            Self::quoted_value(&expansion.access_form)?,
        ]))
    }
}
