use ncl_syntax::{Form, FormKind};

use crate::environment::normalize_name;
use crate::evaluator::helpers::atom_name;
use crate::evaluator::evaluator_literals::resolved_symbol;
use crate::evaluator::{MacroBindingContext, ModifyMacroContext, MAX_MACRO_EXPANSIONS};
use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(in crate::evaluator) fn expand_macros(
        &self,
        form: Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        self.expand_macros_with_flag(form, environment)
            .map(|(form, _)| form)
    }

    pub(in crate::evaluator) fn expand_macros_with_flag(
        &self,
        mut form: Form,
        environment: &Environment,
    ) -> Result<(Form, bool), RuntimeError> {
        let mut expanded_p = false;
        for _ in 0..MAX_MACRO_EXPANSIONS {
            let Some(expanded) = self.expand_macro_once(&form, environment)? else {
                return Ok((form, expanded_p));
            };
            expanded_p = true;
            form = expanded;
        }
        Err(Self::invalid(
            "macro expansion exceeded its limit",
            form.span,
        ))
    }

    pub(in crate::evaluator) fn expand_macro_once(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Option<Form>, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(None);
        };
        let Some(operator) = items.first() else {
            return Ok(None);
        };
        let Some(name) = atom_name(operator) else {
            return Ok(None);
        };
        let (resolved_name, escaped) = resolved_symbol(name);
        let function = if escaped {
            self.lookup_function_exact_in(&resolved_name, environment)
        } else {
            self.lookup_in(&resolved_name, environment)
        };
        let Some(function) = function else {
            if !escaped {
                match normalize_name(&resolved_name).as_str() {
                    "WITH-SLOTS" => {
                        return Self::expand_builtin_with_slots(form, false).map(Some);
                    }
                    "WITH-ACCESSORS" => {
                        return Self::expand_builtin_with_slots(form, true).map(Some);
                    }
                    _ => {}
                }
            }
            return Ok(None);
        };
        let Value::Function(function) = function else {
            return Ok(None);
        };
        let expansion = match function.as_ref() {
            crate::Function::Macro {
                lambda_list,
                body,
                environment: macro_environment,
            } => {
                let expansion = self.invoke_macro(
                    MacroBindingContext {
                        form,
                        arguments: &items[1..],
                        macro_name: name,
                        lambda_list,
                        macro_environment,
                        environment,
                    },
                    body,
                )?;
                let expansion = expansion.primary_value();
                Self::form_from_value(&expansion, form.span)?
            }
            crate::Function::ModifyMacro {
                lambda_list,
                function,
                environment: macro_environment,
            } => self.invoke_modify_macro(&ModifyMacroContext {
                binding: MacroBindingContext {
                    form,
                    arguments: &items[1..],
                    macro_name: name,
                    lambda_list,
                    macro_environment,
                    environment,
                },
                function,
            })?,
            _ => return Ok(None),
        };
        Ok(Some(expansion))
    }
}
