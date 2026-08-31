use super::{
    ClosureApplicationContext, ClosureKeywordApplicationContext, Environment,
    LambdaListAuxiliaryParameter, Runtime, RuntimeError, Value,
};
use crate::environment::names_equal;
use std::collections::HashSet;

mod keywords;

impl Runtime {
    pub(super) fn apply_closure(
        &self,
        context: &ClosureApplicationContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let ClosureApplicationContext {
            parameters,
            required_escaped,
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
        let optional_supplied_count = if has_keyword_section {
            let available = arguments
                .len()
                .saturating_sub(required_count)
                .min(optional_count);
            (0..available)
                .take_while(|index| {
                    !matches!(
                        arguments[required_count + *index],
                        Value::Keyword(_) | Value::KeywordExact(_)
                    )
                })
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

        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let special_parameters = Self::special_parameter_names(body);
        for (index, (parameter, argument)) in parameters.iter().zip(arguments.iter()).enumerate() {
            let escaped = required_escaped.get(index).copied().unwrap_or(false);
            if special_parameters.contains(&(parameter.clone(), escaped)) {
                if escaped {
                    self.define_dynamic_exact(parameter, argument.clone());
                } else {
                    self.define_dynamic(parameter, argument.clone());
                }
            } else if escaped {
                self.define_exact_in(parameter, argument.clone(), &local);
            } else {
                self.define_in(parameter, argument.clone(), &local);
            }
        }
        self.apply_closure_optional(
            optional,
            optional_supplied_count,
            required_count,
            arguments,
            &local,
            &special_parameters,
        )?;
        self.apply_closure_rest(
            rest,
            rest_escaped,
            &arguments[key_start..],
            &local,
            &special_parameters,
        );
        if has_keyword_section {
            self.apply_closure_keywords(&ClosureKeywordApplicationContext {
                keywords,
                arguments,
                key_start,
                allow_other_keys,
                local: &local,
                special_parameters: &special_parameters,
                span,
            })?;
        }
        self.apply_closure_auxiliary(auxiliary, &local, &special_parameters)?;
        self.eval_sequence_values(body, &local)
    }

    fn special_parameter_names(body: &[ncl_syntax::Form]) -> HashSet<(String, bool)> {
        body.iter()
            .take_while(|form| matches!(form.kind, ncl_syntax::FormKind::List(_)))
            .filter_map(|form| {
                let ncl_syntax::FormKind::List(parts) = &form.kind else {
                    return None;
                };
                parts
                    .first()
                    .and_then(crate::evaluator::atom_name)
                    .filter(|name| names_equal(name, "DECLARE"))?;
                Some(parts)
            })
            .flat_map(|parts| parts.iter().skip(1))
            .filter_map(|spec| {
                let ncl_syntax::FormKind::List(parts) = &spec.kind else {
                    return None;
                };
                parts
                    .first()
                    .and_then(crate::evaluator::atom_name)
                    .filter(|name| names_equal(name, "SPECIAL"))?;
                Some(parts.iter().skip(1))
            })
            .flatten()
            .filter_map(|form| Self::variable_name_info(form, "declare special name").ok())
            .collect()
    }

    pub(super) fn define_closure_binding(
        &self,
        name: &str,
        escaped: bool,
        value: Value,
        local: &Environment,
        special_parameters: &HashSet<(String, bool)>,
    ) {
        if special_parameters.contains(&(name.to_owned(), escaped)) {
            if escaped {
                self.define_dynamic_exact(name, value);
            } else {
                self.define_dynamic(name, value);
            }
        } else if escaped {
            self.define_exact_in(name, value, local);
        } else {
            self.define_in(name, value, local);
        }
    }

    fn apply_closure_optional(
        &self,
        optional: &[super::LambdaListOptionalParameter],
        supplied_count: usize,
        required_count: usize,
        arguments: &[Value],
        local: &Environment,
        special_parameters: &HashSet<(String, bool)>,
    ) -> Result<(), RuntimeError> {
        for (index, specification) in optional.iter().enumerate() {
            let supplied = (index < supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None => self.eval_in(&specification.init_form, local)?,
            };
            self.define_closure_binding(
                &specification.name,
                specification.name_escaped,
                value,
                local,
                special_parameters,
            );
            if let Some(supplied_p) = &specification.supplied_p {
                let supplied_value = Value::boolean(supplied.is_some());
                self.define_closure_binding(
                    supplied_p,
                    specification.supplied_p_escaped.unwrap_or(false),
                    supplied_value,
                    local,
                    special_parameters,
                );
            }
        }
        Ok(())
    }

    fn apply_closure_rest(
        &self,
        rest: Option<&String>,
        rest_escaped: bool,
        arguments: &[Value],
        local: &Environment,
        special_parameters: &HashSet<(String, bool)>,
    ) {
        if let Some(rest) = rest {
            let value = Value::list(arguments.to_vec());
            self.define_closure_binding(rest, rest_escaped, value, local, special_parameters);
        }
    }

    fn apply_closure_auxiliary(
        &self,
        auxiliary: &[LambdaListAuxiliaryParameter],
        local: &Environment,
        special_parameters: &HashSet<(String, bool)>,
    ) -> Result<(), RuntimeError> {
        for specification in auxiliary {
            let value = self.eval_in(&specification.init_form, local)?;
            self.define_closure_binding(
                &specification.name,
                specification.name_escaped,
                value,
                local,
                special_parameters,
            );
        }
        Ok(())
    }
}
