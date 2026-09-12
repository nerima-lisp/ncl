use super::{Form, FormKind, Runtime, RuntimeError, SetfExpansion, Span, Value};

impl Runtime {
    pub(super) fn parse_prepared_setf_expansion(
        fields: &[Form],
        span: Span,
    ) -> Result<SetfExpansion, RuntimeError> {
        let [temporaries, values, stores, store_form, access_form] = fields else {
            return Err(Self::invalid("SETF expander must return five values", span));
        };
        let list = |form: &Form| match &form.kind {
            FormKind::List(items) => Ok(items.clone()),
            _ => Err(Self::invalid(
                "prepared SETF expansion field must be a proper list",
                span,
            )),
        };
        let temporaries = list(temporaries)?;
        let values = list(values)?;
        let stores = list(stores)?;
        if temporaries.len() != values.len() {
            return Err(Self::invalid(
                "SETF expansion temporary and value lists must have the same length",
                span,
            ));
        }
        for variable in temporaries.iter().chain(&stores) {
            Self::variable_name_info(variable, "SETF expansion binding must be a symbol")?;
        }
        Ok(SetfExpansion {
            temporaries,
            values,
            stores,
            store_form: store_form.clone(),
            access_form: access_form.clone(),
        })
    }

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
        let stores = Self::setf_expansion_forms(&values[2], "store variables", span)?;
        for store in &stores {
            Self::variable_name_info(store, "SETF store variable must be a symbol")?;
        }
        Ok(SetfExpansion {
            temporaries,
            values: value_forms,
            stores,
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
            list_value(&expansion.stores)?,
            Self::quoted_value(&expansion.store_form)?,
            Self::quoted_value(&expansion.access_form)?,
        ]))
    }
}
