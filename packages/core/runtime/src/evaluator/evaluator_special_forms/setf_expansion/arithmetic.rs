use super::temporary::SetfTemporaryContext;
use super::{Environment, Form, Runtime, RuntimeError};

impl Runtime {
    pub(crate) fn expand_arithmetic_place(
        &self,
        items: &[Form],
        environment: &Environment,
        operator: &str,
        arithmetic: &str,
    ) -> Result<Form, RuntimeError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(Self::arity(
                operator,
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let span = items[0].span;
        let mut context = SetfTemporaryContext::default();
        for item in items {
            context.reserve_form(item, environment);
        }
        let expansion = self.get_modify_macro_setf_expansion_with_context(
            &items[1],
            environment,
            &mut context,
        )?;
        context.reserve_expansion(&expansion, environment);
        if expansion.temporaries.len() != expansion.values.len() {
            return Err(Self::invalid(
                "SETF expansion temporary/value count mismatch",
                span,
            ));
        }
        let mut bindings: Vec<Form> = expansion
            .temporaries
            .into_iter()
            .zip(expansion.values)
            .map(|(temporary, value)| Form::list(vec![temporary, value], span))
            .collect();
        let delta = self.fresh_setf_temporary(span, environment, &mut context);
        bindings.push(Form::list(
            vec![
                delta.clone(),
                items
                    .get(2)
                    .cloned()
                    .unwrap_or_else(|| Form::atom("1", span)),
            ],
            span,
        ));
        let update = Form::list(
            vec![
                Form::atom("MULTIPLE-VALUE-BIND", span),
                Form::list(expansion.stores.clone(), span),
                Form::list(
                    vec![Form::atom(arithmetic, span), expansion.access_form, delta],
                    span,
                ),
                expansion.store_form,
                Self::setf_store_values_form(&expansion.stores, span),
            ],
            span,
        );
        Ok(Form::list(
            vec![Form::atom("LET*", span), Form::list(bindings, span), update],
            span,
        ))
    }
}
