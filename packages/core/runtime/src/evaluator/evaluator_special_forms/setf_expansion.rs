#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn fresh_setf_temporary(&self, span: Span) -> Form {
        let counter = self.gensym_counter.get();
        self.gensym_counter.set(counter.wrapping_add(1));
        Form::atom(format!("NCL-SETF-TEMP-{counter}"), span)
    }

    pub(super) fn setf_expansion_forms(
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

    pub(super) fn setf_expansion_value(
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

    pub(super) fn custom_setf_expansion(
        &self,
        place: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Option<SetfExpansion>, RuntimeError> {
        let Some(operator) = items.first().and_then(atom_name) else {
            return Ok(None);
        };
        let lookup_name = unqualified_name(operator);
        let Some(function) = environment.lookup_setf_expander(&lookup_name) else {
            return Ok(None);
        };
        let Value::Function(function) = function else {
            return Err(Self::invalid("SETF expander is not a function", place.span));
        };
        let crate::Function::Macro {
            lambda_list,
            body,
            environment: macro_environment,
        } = function.as_ref()
        else {
            return Err(Self::invalid(
                "SETF expander is not a macro function",
                place.span,
            ));
        };
        let expansion = self.invoke_macro(
            MacroBindingContext {
                form: place,
                arguments: &items[1..],
                macro_name: operator,
                lambda_list,
                macro_environment,
                environment,
            },
            body,
        )?;
        Ok(Some(Self::parse_setf_expansion(&expansion, place.span)?))
    }

    pub(super) fn get_setf_expansion(
        &self,
        place: &Form,
        environment: &Environment,
    ) -> Result<SetfExpansion, RuntimeError> {
        if let Some(expanded) = Self::expand_symbol_macro_form(place, environment)? {
            return self.get_setf_expansion(&expanded, environment);
        }
        if atom_name(place).is_some() {
            Self::variable_name_info(place, "SETF place must be a symbol")?;
            let store = self.fresh_setf_temporary(place.span);
            let store_form = Form::list(
                vec![Form::atom("SETQ", place.span), place.clone(), store.clone()],
                place.span,
            );
            return Ok(SetfExpansion {
                temporaries: Vec::new(),
                values: Vec::new(),
                store,
                store_form,
                access_form: place.clone(),
            });
        }

        let FormKind::List(items) = &place.kind else {
            return Err(Self::invalid("unsupported SETF place", place.span));
        };
        let Some(_operator) = items.first().and_then(atom_name) else {
            return Err(Self::invalid("unsupported SETF place", place.span));
        };
        if let Some(expansion) = self.custom_setf_expansion(place, items, environment)? {
            return Ok(expansion);
        }

        let temporaries = items[1..]
            .iter()
            .map(|_| self.fresh_setf_temporary(place.span))
            .collect::<Vec<_>>();
        let values = items[1..].to_vec();
        let store = self.fresh_setf_temporary(place.span);
        let mut access_items = Vec::with_capacity(items.len());
        access_items.push(items[0].clone());
        access_items.extend(temporaries.iter().cloned());
        let access_form = Form::list(access_items, place.span);
        let store_form = Form::list(
            vec![
                Form::atom("SETF", place.span),
                access_form.clone(),
                store.clone(),
            ],
            place.span,
        );
        Ok(SetfExpansion {
            temporaries,
            values,
            store,
            store_form,
            access_form,
        })
    }

    pub(crate) fn get_modify_macro_setf_expansion(
        &self,
        place: &Form,
        environment: &Environment,
    ) -> Result<SetfExpansion, RuntimeError> {
        if let Some(expanded) = Self::expand_symbol_macro_form(place, environment)? {
            return self.get_modify_macro_setf_expansion(&expanded, environment);
        }
        if atom_name(place).is_some() {
            return self.get_setf_expansion(place, environment);
        }

        let FormKind::List(items) = &place.kind else {
            return Err(Self::invalid("unsupported SETF place", place.span));
        };
        let Some(operator) = items.first().and_then(atom_name) else {
            return Err(Self::invalid("unsupported SETF place", place.span));
        };
        if let Some(expansion) = self.custom_setf_expansion(place, items, environment)? {
            return Ok(expansion);
        }
        let Some(container_index) =
            Self::modify_macro_container_index(operator, items.len().saturating_sub(1))
        else {
            return self.get_setf_expansion(place, environment);
        };

        let outer_temporaries = items[1..]
            .iter()
            .map(|_| self.fresh_setf_temporary(place.span))
            .collect::<Vec<_>>();
        let outer_values = items[1..].to_vec();
        let nested =
            self.get_modify_macro_setf_expansion(&outer_values[container_index], environment)?;

        let mut temporaries = Vec::new();
        let mut values = Vec::new();
        for (index, (temporary, value_form)) in outer_temporaries
            .iter()
            .zip(outer_values.iter())
            .enumerate()
        {
            if index == container_index {
                temporaries.extend(nested.temporaries.iter().cloned());
                values.extend(nested.values.iter().cloned());
                temporaries.push(temporary.clone());
                values.push(nested.access_form.clone());
            } else {
                temporaries.push(temporary.clone());
                values.push(value_form.clone());
            }
        }

        let mut access_items = Vec::with_capacity(items.len());
        access_items.push(items[0].clone());
        access_items.extend(outer_temporaries.iter().cloned());
        let access_form = Form::list(access_items, place.span);
        let store = self.fresh_setf_temporary(place.span);
        let outer_store_form = Form::list(
            vec![
                Form::atom("SETF", place.span),
                access_form.clone(),
                store.clone(),
            ],
            place.span,
        );
        let nested_store_form = Form::list(
            vec![
                Form::atom("LET", place.span),
                Form::list(
                    vec![Form::list(
                        vec![
                            nested.store.clone(),
                            outer_temporaries[container_index].clone(),
                        ],
                        place.span,
                    )],
                    place.span,
                ),
                nested.store_form.clone(),
            ],
            place.span,
        );
        let store_form = Form::list(
            vec![
                Form::atom("PROGN", place.span),
                outer_store_form,
                nested_store_form,
            ],
            place.span,
        );

        Ok(SetfExpansion {
            temporaries,
            values,
            store,
            store_form,
            access_form,
        })
    }

    pub(super) fn modify_macro_container_index(
        operator: &str,
        argument_count: usize,
    ) -> Option<usize> {
        let index = match unqualified_name(operator).as_str() {
            "CAR" | "FIRST" | "CDR" | "REST" | "GETF" | "ELT" | "CHAR" | "SCHAR" | "BIT"
            | "AREF" | "ROW-MAJOR-AREF" | "SVREF" | "SUBSEQ" => 0,
            "NTH" => 1,
            _ => return None,
        };
        (index < argument_count).then_some(index)
    }

    pub(super) fn apply_setf_expansion(
        &self,
        expansion: &SetfExpansion,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if expansion.temporaries.len() != expansion.values.len() {
            return Err(Self::invalid(
                "SETF expansion temporary and value lists must have the same length",
                span,
            ));
        }
        let local = environment.child();
        for (temporary, value_form) in expansion.temporaries.iter().zip(&expansion.values) {
            let (name, escaped) =
                Self::variable_name_info(temporary, "SETF temporary must be a symbol")?;
            let value = self.eval_in(value_form, &local)?;
            self.define_variable_in(&name, escaped, value, &local);
        }
        let (store_name, store_escaped) =
            Self::variable_name_info(&expansion.store, "SETF store variable must be a symbol")?;
        self.define_variable_in(&store_name, store_escaped, value, &local);
        self.eval_in(&expansion.store_form, &local)?;
        Ok(())
    }
}
