use ncl_syntax::{Form, Span};

use crate::evaluator::ModifyMacroContext;
use crate::evaluator::evaluator_state::SetfExpansion;
use crate::evaluator::helpers::is_operator_form;
use crate::value::{MacroLambdaList, MacroPattern};
use crate::{Environment, Runtime, RuntimeError};

impl Runtime {
    fn build_modify_macro_call(
        &self,
        function: &Form,
        lambda_list: &MacroLambdaList,
        local: &Environment,
        expansion: &SetfExpansion,
        form_span: Span,
    ) -> Result<Form, RuntimeError> {
        let function_designator = if is_operator_form(function, "FUNCTION") {
            function.clone()
        } else {
            Form::list(
                vec![Form::atom("FUNCTION", function.span), function.clone()],
                function.span,
            )
        };
        let mut call_items = vec![
            Form::atom("FUNCALL", form_span),
            function_designator,
            expansion.access_form.clone(),
        ];
        for pattern in lambda_list.required.iter().skip(1) {
            let MacroPattern::Name(name) = pattern else {
                return Err(Self::invalid(
                    "define-modify-macro required parameters must be names",
                    form_span,
                ));
            };
            let value = self.lookup_in(name, local).ok_or_else(|| {
                Self::invalid("define-modify-macro parameter is unbound", form_span)
            })?;
            call_items.push(Self::form_from_value(&value, form_span)?);
        }
        for specification in &lambda_list.optional {
            let MacroPattern::Name(name) = &specification.pattern else {
                return Err(Self::invalid(
                    "define-modify-macro optional parameters must be names",
                    form_span,
                ));
            };
            let value = self.lookup_in(name, local).ok_or_else(|| {
                Self::invalid("define-modify-macro parameter is unbound", form_span)
            })?;
            call_items.push(Self::form_from_value(&value, form_span)?);
        }
        if let Some(rest_name) = &lambda_list.rest {
            let rest_value = self.lookup_in(rest_name, local).ok_or_else(|| {
                Self::invalid("define-modify-macro rest parameter is unbound", form_span)
            })?;
            let rest_values = rest_value.list_items().ok_or_else(|| {
                Self::invalid(
                    "define-modify-macro rest parameter is not a list",
                    form_span,
                )
            })?;
            for value in rest_values {
                call_items.push(Self::form_from_value(&value, form_span)?);
            }
        } else if lambda_list.has_keyword_section {
            for specification in &lambda_list.keywords {
                let MacroPattern::Name(name) = &specification.pattern else {
                    return Err(Self::invalid(
                        "define-modify-macro keyword parameters must be names",
                        form_span,
                    ));
                };
                let value = self.lookup_in(name, local).ok_or_else(|| {
                    Self::invalid(
                        "define-modify-macro keyword parameter is unbound",
                        form_span,
                    )
                })?;
                call_items.push(Form::atom(
                    format!(":{}", specification.keyword_name),
                    form_span,
                ));
                call_items.push(Self::form_from_value(&value, form_span)?);
            }
        }
        Ok(Form::list(call_items, form_span))
    }

    pub(super) fn invoke_modify_macro(
        &self,
        context: &ModifyMacroContext<'_>,
    ) -> Result<Form, RuntimeError> {
        let ModifyMacroContext { binding, function } = *context;
        let form = binding.form;
        let lambda_list = binding.lambda_list;
        let environment = binding.environment;
        let local = self.bind_macro_arguments(binding)?;
        let Some(MacroPattern::Name(place_name)) = lambda_list.required.first() else {
            return Err(Self::invalid(
                "define-modify-macro requires a place parameter",
                form.span,
            ));
        };
        let place_value = self.lookup_in(place_name, &local).ok_or_else(|| {
            Self::invalid(
                "define-modify-macro could not bind its place parameter",
                form.span,
            )
        })?;
        let place = Self::form_from_value(&place_value, form.span)?;
        let expansion = self.get_modify_macro_setf_expansion(&place, environment)?;

        let call =
            self.build_modify_macro_call(function, lambda_list, &local, &expansion, form.span)?;
        let store_binding = Form::list(vec![expansion.store.clone(), call], form.span);
        let update = Form::list(
            vec![
                Form::atom("LET", form.span),
                Form::list(vec![store_binding], form.span),
                Form::list(
                    vec![
                        Form::atom("PROGN", form.span),
                        expansion.store_form.clone(),
                        expansion.store.clone(),
                    ],
                    form.span,
                ),
            ],
            form.span,
        );
        let temporary_bindings = expansion
            .temporaries
            .iter()
            .zip(expansion.values.iter())
            .map(|(temporary, value)| Form::list(vec![temporary.clone(), value.clone()], form.span))
            .collect();
        Ok(Form::list(
            vec![
                Form::atom("LET*", form.span),
                Form::list(temporary_bindings, form.span),
                update,
            ],
            form.span,
        ))
    }
}
