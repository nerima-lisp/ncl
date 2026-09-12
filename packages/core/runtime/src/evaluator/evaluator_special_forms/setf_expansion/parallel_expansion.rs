use super::temporary::SetfTemporaryContext;
use super::{
    Environment, Form, FormKind, Runtime, RuntimeError, SetfExpansion, Value, atom_name,
    is_special_form, unqualified_name,
};

impl Runtime {
    pub(in crate::evaluator) fn parallel_setf_expansion_for_compilation(
        &self,
        place: &Form,
        environment: &Environment,
    ) -> Result<SetfExpansion, RuntimeError> {
        self.parallel_setf_expansion(place, environment, &mut SetfTemporaryContext::default())
    }

    pub(super) fn parallel_setf_expansion(
        &self,
        place: &Form,
        environment: &Environment,
        context: &mut SetfTemporaryContext,
    ) -> Result<SetfExpansion, RuntimeError> {
        context.reserve_form(place, environment);
        if let Some(expanded) = Self::expand_symbol_macro_form(place, environment)? {
            return self.parallel_setf_expansion(&expanded, environment, context);
        }
        let FormKind::List(items) = &place.kind else {
            return self.get_setf_expansion_with_context(place, environment, context);
        };
        let Some(operator) = items.first().and_then(atom_name) else {
            return self.get_setf_expansion_with_context(place, environment, context);
        };
        let name = unqualified_name(operator);
        if items.len() < 2
            || !matches!(name.as_str(), "GETF" | "ELT" | "CHAR" | "SCHAR" | "SUBSEQ")
            || environment.lookup_setf_expander(&name).is_some()
        {
            return self.get_setf_expansion_with_context(place, environment, context);
        }

        let span = place.span;
        let nested = if name == "GETF" {
            self.parallel_setf_expansion(&items[1], environment, context)?
        } else {
            self.parallel_container_expansion(&items[1], environment, context)?
        };
        context.reserve_expansion(&nested, environment);
        let mut outer = self.get_setf_expansion_with_context(place, environment, context)?;
        let container = outer.temporaries[0].clone();
        let mut temporaries = nested.temporaries;
        let mut values = nested.values;
        let late_read = name == "GETF";
        let saved = self.fresh_setf_temporary(span, environment, context);
        if !late_read {
            temporaries.extend([container.clone(), saved.clone()]);
            values.extend([nested.access_form.clone(), container.clone()]);
        }
        temporaries.extend(outer.temporaries.into_iter().skip(1));
        values.extend(outer.values.into_iter().skip(1));

        let copyback = Form::list(
            vec![
                Form::atom("MULTIPLE-VALUE-BIND", span),
                Form::list(nested.stores, span),
                container.clone(),
                nested.store_form,
            ],
            span,
        );
        outer.store_form = if late_read {
            // GETF reads its enclosing place after the new value is evaluated.
            if let FormKind::List(access_items) = &mut outer.access_form.kind {
                access_items[1] = nested.access_form.clone();
            }
            Form::list(
                vec![
                    Form::atom("LET", span),
                    Form::list(
                        vec![Form::list(vec![container, nested.access_form], span)],
                        span,
                    ),
                    outer.store_form,
                    copyback,
                    Self::setf_store_values_form(&outer.stores, span),
                ],
                span,
            )
        } else {
            // Mutable containers retain their identity; only reconstructed strings need copyback.
            Form::list(
                vec![
                    Form::atom("PROGN", span),
                    outer.store_form,
                    Form::list(
                        vec![
                            Form::atom("IF", span),
                            Form::list(vec![Form::atom("EQ", span), saved, container], span),
                            Form::atom("NIL", span),
                            copyback,
                        ],
                        span,
                    ),
                    Self::setf_store_values_form(&outer.stores, span),
                ],
                span,
            )
        };
        outer.temporaries = temporaries;
        outer.values = values;
        Ok(outer)
    }

    fn parallel_container_expansion(
        &self,
        form: &Form,
        environment: &Environment,
        context: &mut SetfTemporaryContext,
    ) -> Result<SetfExpansion, RuntimeError> {
        let expanded = Self::expand_symbol_macro_form(form, environment)?;
        let form = expanded.as_ref().unwrap_or(form);
        let computed = match &form.kind {
            FormKind::Atom(_) => false,
            FormKind::List(forms) => forms.first().is_none_or(|head| {
                atom_name(head).is_none_or(|operator| {
                    environment.lookup_setf_expander(&unqualified_name(operator)).is_none()
                        && (is_special_form(head)
                            || matches!(self.lookup_function_in(operator, environment),
                                Some(Value::Function(function)) if matches!(function.as_ref(), crate::Function::Macro { .. })))
                })
            }),
            _ => true,
        };
        if !computed {
            return self.parallel_setf_expansion(form, environment, context);
        }
        context.reserve_form(form, environment);
        let span = form.span;
        let store = self.fresh_setf_temporary(span, environment, context);
        Ok(SetfExpansion {
            temporaries: Vec::new(),
            values: Vec::new(),
            stores: vec![store.clone()],
            store_form: Form::list(
                vec![
                    Form::atom("%SETF-INTRINSIC-STORE", span),
                    form.clone(),
                    store,
                ],
                span,
            ),
            access_form: form.clone(),
        })
    }
}
