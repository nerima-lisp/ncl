#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn eval_in(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_values_in(form, environment)
            .map(|value| value.primary_value())
    }

    pub(crate) fn eval_values_in(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        match &form.kind {
            FormKind::Atom(atom) => {
                if let Some(expanded) = Self::expand_symbol_macro_form(form, environment)? {
                    return self.eval_values_in(&expanded, environment);
                }
                self.eval_atom(atom, form.span, environment)
            }
            FormKind::String(value) => Ok(Value::string(value.clone())),
            FormKind::Character(value) => Ok(Value::Character(*value)),
            FormKind::Vector(items) => Ok(Value::vector(
                items
                    .iter()
                    .map(Self::quoted_value)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            FormKind::DottedList { .. } => {
                Err(Self::invalid("cannot evaluate a dotted list", form.span))
            }
            FormKind::List(items) => self.eval_list_values(items, form.span, environment),
        }
    }

    fn eval_atom(
        &self,
        atom: &str,
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if let Some(value) = literal_atom(atom) {
            return Ok(value);
        }
        let (name, escaped) = resolved_symbol(atom);
        let value = if escaped {
            self.lookup_exact_in(&name, environment)
        } else {
            self.lookup_in(&name, environment)
        };
        value.ok_or_else(|| RuntimeError::UnboundVariable {
            name: normalize_name(&name),
            span: Some(span),
        })
    }

    fn eval_list_values(
        &self,
        items: &[Form],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let form = Form::list(items.to_vec(), span);
        let expanded = self.expand_macros(form, environment)?;
        self.eval_expanded_values(&expanded, environment)
    }

    pub(super) fn eval_expanded_values(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return self.eval_values_in(form, environment);
        };
        let Some(operator) = items.first() else {
            return Ok(Value::Nil);
        };
        if let Some(name) = atom_name(operator) {
            let escaped = parse_symbol_token(name).is_ok_and(|token| token.escaped);
            if !escaped {
                if let Some(value) = self.eval_special_form_core(form, items, name, environment)? {
                    return Ok(value);
                }
                if let Some(value) =
                    self.eval_special_form_conditionals(items, name, environment)?
                {
                    return Ok(value);
                }
                if let Some(value) = self.eval_special_form_bindings(items, name, environment)? {
                    return Ok(value);
                }
                if let Some(value) = self.eval_special_form_iteration(items, name, environment)? {
                    return Ok(value);
                }
                if let Some(value) = self.eval_special_form_functions(items, name, environment)? {
                    return Ok(value);
                }
                if let Some(value) = self.eval_special_form_macros(items, name, environment)? {
                    return Ok(value);
                }
                if let Some(value) = self.eval_special_form_expansion(items, name, environment)? {
                    return Ok(value);
                }
                if let Some(value) = self.eval_special_form_mutation(items, name, environment)? {
                    return Ok(value);
                }
            }
        }

        self.eval_function_form(operator, &items[1..], form.span, environment)
    }

    fn eval_function_form(
        &self,
        operator: &Form,
        argument_forms: &[Form],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let function = if let Some(name) = atom_name(operator) {
            let (resolved_name, escaped) = resolved_symbol(name);
            let function = if escaped {
                self.lookup_function_exact_in(&resolved_name, environment)
            } else {
                self.lookup_function_in(&resolved_name, environment)
            };
            function.ok_or_else(|| RuntimeError::UnboundVariable {
                name: if escaped {
                    resolved_name
                } else {
                    normalize_name(&resolved_name)
                },
                span: Some(operator.span),
            })?
        } else {
            self.eval_in(operator, environment)?
        };
        let arguments = argument_forms
            .iter()
            .map(|item| self.eval_in(item, environment))
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_in(&function, &arguments, span, environment)
    }
}
