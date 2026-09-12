use ncl_syntax::{Form, Span};

use crate::evaluator::ModifyMacroContext;
use crate::evaluator::helpers::is_operator_form;
use crate::value::{MacroLambdaList, MacroPattern};
use crate::{Environment, Runtime, RuntimeError};

impl Runtime {
    fn modify_macro_arguments(
        &self,
        lambda_list: &MacroLambdaList,
        local: &Environment,
        form_span: Span,
    ) -> Result<Vec<Form>, RuntimeError> {
        let mut call_items = Vec::new();
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
        Ok(call_items)
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
        let arguments = self.modify_macro_arguments(lambda_list, &local, form.span)?;
        let function_designator = if is_operator_form(function, "FUNCTION") {
            function.clone()
        } else {
            Form::list(
                vec![Form::atom("FUNCTION", function.span), function.clone()],
                function.span,
            )
        };
        self.expand_modify_macro_place(form, &place, &function_designator, &arguments, environment)
    }
}
