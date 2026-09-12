use super::temporary::SetfTemporaryContext;
use super::{Environment, Form, Runtime, RuntimeError, SetfExpansion, Span, Value};

impl Runtime {
    pub(super) fn values_setf_expansion(
        &self,
        place: &Form,
        places: &[Form],
        environment: &Environment,
        context: &mut SetfTemporaryContext,
    ) -> Result<SetfExpansion, RuntimeError> {
        let span = place.span;
        let mut expansion = SetfExpansion {
            temporaries: Vec::new(),
            values: Vec::new(),
            stores: Vec::new(),
            store_form: Form::atom("NIL", span),
            access_form: Form::atom("NIL", span),
        };
        let mut writes = vec![Form::atom("PROGN", span)];
        let mut reads = vec![Form::atom("VALUES", span)];
        let mut results = Vec::new();
        for child in places {
            let nested = self.parallel_setf_expansion(child, environment, context)?;
            context.reserve_expansion(&nested, environment);
            expansion.temporaries.extend(nested.temporaries);
            expansion.values.extend(nested.values);
            let store = nested.stores.first().cloned();
            expansion.stores.extend(store.iter().cloned());
            let store = store.unwrap_or_else(|| Form::atom("NIL", span));
            results.push(store.clone());
            writes.push(Form::list(
                vec![
                    Form::atom("MULTIPLE-VALUE-BIND", span),
                    Form::list(nested.stores, span),
                    store,
                    nested.store_form,
                ],
                span,
            ));
            reads.push(nested.access_form);
        }
        writes.push(Self::setf_store_values_form(&results, span));
        expansion.store_form = Form::list(writes, span);
        expansion.access_form = Form::list(reads, span);
        Ok(expansion)
    }

    pub(super) fn setf_store_values_form(stores: &[Form], span: Span) -> Form {
        let mut forms = vec![Form::atom("VALUES", span)];
        forms.extend_from_slice(stores);
        Form::list(forms, span)
    }

    pub(in crate::evaluator::evaluator_special_forms) fn bind_setf_stores(
        &self,
        stores: &[Form],
        value: &Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        let names = stores
            .iter()
            .map(|store| Self::variable_name_info(store, "SETF store variable must be a symbol"))
            .collect::<Result<Vec<_>, _>>()?;
        let values = value.multiple_values();
        for ((name, escaped), value) in names
            .into_iter()
            .zip(values.into_iter().chain(std::iter::repeat(Value::Nil)))
        {
            self.define_variable_in(&name, escaped, value, environment);
        }
        Ok(())
    }
}
