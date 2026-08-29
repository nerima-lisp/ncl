use super::{
    Environment, Form, FormKind, MacroBindingContext, Runtime, RuntimeError, SetfExpansion, Value,
    atom_name, unqualified_name,
};

impl Runtime {
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

    pub(in crate::evaluator::evaluator_special_forms) fn get_setf_expansion(
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_setf_expansion_rejects_a_non_function_expander() {
        let runtime = Runtime::new();
        let environment = runtime.global_environment();
        environment.define_setf_expander("NOT-A-FUNCTION-PLACE", Value::Integer(1));

        let error = runtime
            .eval_source("(setf (not-a-function-place) 1)")
            .map_or_else(
                |error| error,
                |value| panic!("a non-function expander must be rejected, got {value:?}"),
            );

        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "SETF expander is not a function"
        ));
    }

    #[test]
    fn custom_setf_expansion_rejects_a_non_macro_function_expander() {
        let runtime = Runtime::new();
        let environment = runtime.global_environment();
        let car_function = runtime
            .lookup_function_in("CAR", &environment)
            .unwrap_or_else(|| panic!("CAR must be a builtin function"));
        environment.define_setf_expander("NOT-A-MACRO-PLACE", car_function);

        let error = runtime
            .eval_source("(setf (not-a-macro-place) 1)")
            .map_or_else(
                |error| error,
                |value| panic!("a non-macro-function expander must be rejected, got {value:?}"),
            );

        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "SETF expander is not a macro function"
        ));
    }
}
