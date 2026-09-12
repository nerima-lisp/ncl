use super::temporary::SetfTemporaryContext;
use super::{Environment, Form, Runtime, RuntimeError, SetfExpansion};

impl Runtime {
    pub(crate) fn expand_modify_macro_place(
        &self,
        invocation: &Form,
        place: &Form,
        function: &Form,
        arguments: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let span = invocation.span;
        let mut context = SetfTemporaryContext::default();
        context.reserve_form(invocation, environment);
        context.reserve_form(function, environment);
        for argument in arguments {
            context.reserve_form(argument, environment);
        }
        let expansion =
            self.get_modify_macro_setf_expansion_with_context(place, environment, &mut context)?;
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
        let mut call = vec![
            Form::atom("FUNCALL", span),
            function.clone(),
            expansion.access_form,
        ];
        for argument in arguments {
            let temporary = self.fresh_setf_temporary(span, environment, &mut context);
            bindings.push(Form::list(vec![temporary.clone(), argument.clone()], span));
            call.push(temporary);
        }
        let update = Form::list(
            vec![
                Form::atom("MULTIPLE-VALUE-BIND", span),
                Form::list(expansion.stores.clone(), span),
                Form::list(call, span),
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

    pub(super) fn get_modify_macro_setf_expansion_with_context(
        &self,
        place: &Form,
        environment: &Environment,
        context: &mut SetfTemporaryContext,
    ) -> Result<SetfExpansion, RuntimeError> {
        self.parallel_setf_expansion(place, environment, context)
    }
}
