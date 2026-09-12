use super::{
    ClosureApplicationContext, ClosureKeywordApplicationContext, Environment,
    LambdaListAuxiliaryParameter, Runtime, RuntimeError, Value,
};
use ncl_syntax::{Form, FormKind};

mod keywords;

impl Runtime {
    pub(super) fn apply_closure(
        &self,
        context: &ClosureApplicationContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let ClosureApplicationContext {
            parameters,
            optional,
            rest,
            rest_escaped,
            keywords,
            has_keyword_section,
            allow_other_keys,
            auxiliary,
            body,
            environment,
            arguments,
            span,
            required_escaped: _,
        } = *context;
        let required_count = parameters.len();
        let optional_count = optional.len();
        let maximum_count = required_count + optional_count;
        if arguments.len() < required_count {
            let expected = if optional_count > 0 || rest.is_some() || has_keyword_section {
                format!("at least {required_count}")
            } else {
                required_count.to_string()
            };
            return Err(Self::arity("closure", &expected, arguments.len()));
        }
        let is_declared_keyword = |argument: &Value| match argument {
            Value::Keyword(name) | Value::KeywordExact(name) => keywords
                .iter()
                .any(|specification| specification.keyword_name == name.to_string()),
            Value::InternedSymbol(symbol) if symbol.keyword() => keywords
                .iter()
                .any(|specification| specification.keyword_name == symbol.name()),
            _ => false,
        };
        let optional_supplied_count = if has_keyword_section && allow_other_keys {
            let available = arguments
                .len()
                .saturating_sub(required_count)
                .min(optional_count);
            (0..available)
                .take_while(|index| !is_declared_keyword(&arguments[required_count + *index]))
                .count()
        } else {
            arguments
                .len()
                .saturating_sub(required_count)
                .min(optional_count)
        };
        let key_start = required_count + optional_supplied_count;
        if !has_keyword_section && rest.is_none() && arguments.len() > maximum_count {
            let expected = if optional_count > 0 {
                format!("at most {maximum_count}")
            } else {
                maximum_count.to_string()
            };
            return Err(Self::arity("closure", &expected, arguments.len()));
        }

        let body = Self::function_body_forms(body);
        let special_names = Self::special_declaration_names(&body)?;
        let mut local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        self.apply_closure_required(context, &local, &special_names);
        self.apply_closure_optional(context, optional_supplied_count, &mut local, &special_names)?;
        self.apply_closure_rest(
            rest,
            rest_escaped,
            &arguments[key_start..],
            &mut local,
            &special_names,
        );
        if has_keyword_section {
            local = self.apply_closure_keywords(&ClosureKeywordApplicationContext {
                keywords,
                arguments,
                key_start,
                allow_other_keys,
                local: &local,
                span,
                special_names: &special_names,
            })?;
        }
        self.apply_closure_auxiliary(auxiliary, &mut local, &special_names)?;
        let local = local.child();
        Self::declare_special_names(&local, &special_names);
        self.eval_sequence_values(&body, &local)
    }

    fn is_declare_form(form: &Form) -> bool {
        let FormKind::List(items) = &form.kind else {
            return false;
        };
        let Some(operator) = items.first() else {
            return false;
        };
        Self::variable_name_info(operator, "declaration operator")
            .ok()
            .is_some_and(|(name, _)| name == "DECLARE")
    }

    pub(crate) fn function_body_forms(body: &[Form]) -> Vec<Form> {
        let mut result = Vec::with_capacity(body.len());
        let mut in_prologue = true;
        let mut documentation_seen = false;

        for (index, form) in body.iter().enumerate() {
            if in_prologue
                && !documentation_seen
                && Self::form_string(form).is_some()
                && body.get(index + 1).is_some_and(Self::is_declare_form)
            {
                documentation_seen = true;
                continue;
            }
            if in_prologue && Self::is_declare_form(form) {
                result.push(form.clone());
                continue;
            }
            in_prologue = false;
            result.push(form.clone());
        }
        result
    }

    fn apply_closure_required(
        &self,
        context: &ClosureApplicationContext<'_>,
        local: &Environment,
        special_names: &[(String, bool)],
    ) {
        for (index, (parameter, argument)) in context
            .parameters
            .iter()
            .zip(context.arguments.iter())
            .enumerate()
        {
            let escaped = context
                .required_escaped
                .get(index)
                .copied()
                .unwrap_or(false);
            if Self::declares_special(special_names, parameter, escaped) {
                Self::declare_special_names(local, &[(parameter.clone(), escaped)]);
            }
            if escaped {
                self.define_exact_in(parameter, argument.clone(), local);
            } else {
                self.define_in(parameter, argument.clone(), local);
            }
        }
    }

    fn apply_closure_optional(
        &self,
        context: &ClosureApplicationContext<'_>,
        optional_supplied_count: usize,
        local: &mut Environment,
        special_names: &[(String, bool)],
    ) -> Result<(), RuntimeError> {
        for (index, specification) in context.optional.iter().enumerate() {
            let supplied = (index < optional_supplied_count)
                .then(|| &context.arguments[context.parameters.len() + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None => self.eval_in(&specification.init_form, local)?,
            };
            *local = local.child();
            if Self::declares_special(
                special_names,
                &specification.name,
                specification.name_escaped,
            ) {
                Self::declare_special_names(
                    local,
                    &[(specification.name.clone(), specification.name_escaped)],
                );
            }
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value, local);
            } else {
                self.define_in(&specification.name, value, local);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                let supplied_value = Value::boolean(supplied.is_some());
                let supplied_p_escaped = specification.supplied_p_escaped.unwrap_or(false);
                if Self::declares_special(special_names, supplied_p, supplied_p_escaped) {
                    Self::declare_special_names(local, &[(supplied_p.clone(), supplied_p_escaped)]);
                }
                if supplied_p_escaped {
                    self.define_exact_in(supplied_p, supplied_value, local);
                } else {
                    self.define_in(supplied_p, supplied_value, local);
                }
            }
        }
        Ok(())
    }

    fn apply_closure_rest(
        &self,
        rest: Option<&String>,
        rest_escaped: bool,
        arguments: &[Value],
        local: &mut Environment,
        special_names: &[(String, bool)],
    ) {
        if let Some(rest) = rest {
            let value = Value::list(arguments.to_vec());
            *local = local.child();
            if Self::declares_special(special_names, rest, rest_escaped) {
                Self::declare_special_names(local, &[(rest.clone(), rest_escaped)]);
            }
            if rest_escaped {
                self.define_exact_in(rest, value, local);
            } else {
                self.define_in(rest, value, local);
            }
        }
    }

    fn apply_closure_auxiliary(
        &self,
        auxiliary: &[LambdaListAuxiliaryParameter],
        local: &mut Environment,
        special_names: &[(String, bool)],
    ) -> Result<(), RuntimeError> {
        for specification in auxiliary {
            let value = self.eval_in(&specification.init_form, local)?;
            *local = local.child();
            if Self::declares_special(
                special_names,
                &specification.name,
                specification.name_escaped,
            ) {
                Self::declare_special_names(
                    local,
                    &[(specification.name.clone(), specification.name_escaped)],
                );
            }
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value, local);
            } else {
                self.define_in(&specification.name, value, local);
            }
        }
        Ok(())
    }
}
