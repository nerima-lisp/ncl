#![allow(clippy::wildcard_imports)]
use super::*;
use crate::environment::names_equal;

impl Runtime {
    pub(super) fn prepare_lambda(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        self.prepare_lambda_at(prepared, 1, 2, environment)
    }

    pub(super) fn prepare_defun(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        self.prepare_lambda_at(prepared, 2, 3, environment)
    }

    fn prepare_lambda_at(
        &self,
        prepared: &mut [Form],
        parameter_index: usize,
        body_index: usize,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if prepared.len() <= parameter_index {
            return self.prepare_tail(prepared, body_index, environment);
        }
        let parameter_form = prepared[parameter_index].clone();
        let local = Self::prepare_compiled_lambda_environment(&parameter_form, environment)?;
        prepared[parameter_index] = self.prepare_compiled_lambda_list(&parameter_form, &local)?;
        self.prepare_tail(prepared, body_index, &local)
    }

    fn prepare_compiled_lambda_environment(
        form: &Form,
        environment: &Environment,
    ) -> Result<Environment, RuntimeError> {
        let lambda_list = match Self::parameters(form) {
            Ok(lambda_list) => lambda_list,
            Err(RuntimeError::InvalidForm { .. }) => return Ok(environment.child()),
            Err(error) => return Err(error),
        };
        let local = environment.child();
        let define = |name: &str, escaped: bool| {
            if escaped {
                local.define_exact(name, Value::Nil);
            } else {
                local.define(name, Value::Nil);
            }
        };

        for (name, escaped) in lambda_list
            .required
            .iter()
            .zip(lambda_list.required_escaped.iter().copied())
        {
            define(name, escaped);
        }
        for parameter in &lambda_list.optional {
            define(&parameter.name, parameter.name_escaped);
            if let Some(name) = &parameter.supplied_p {
                define(name, parameter.supplied_p_escaped.unwrap_or(false));
            }
        }
        if let Some(name) = &lambda_list.rest {
            define(name, lambda_list.rest_escaped);
        }
        for parameter in &lambda_list.keywords {
            define(&parameter.name, parameter.name_escaped);
            if let Some(name) = &parameter.supplied_p {
                define(name, parameter.supplied_p_escaped.unwrap_or(false));
            }
        }
        for parameter in &lambda_list.auxiliary {
            define(&parameter.name, parameter.name_escaped);
        }
        Ok(local)
    }

    pub(super) fn prepare_compiled_lambda_list(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(parameters) = &form.kind else {
            return Ok(form.clone());
        };

        let mut prepared = parameters.clone();
        let mut default_section = false;
        for (index, parameter) in parameters.iter().enumerate() {
            if let Some(name) = atom_name(parameter) {
                if names_equal(name, "&OPTIONAL")
                    || names_equal(name, "&KEY")
                    || names_equal(name, "&AUX")
                {
                    default_section = true;
                } else if names_equal(name, "&REST") {
                    default_section = false;
                }
                continue;
            }
            if !default_section {
                continue;
            }
            let FormKind::List(specification) = &parameter.kind else {
                continue;
            };
            if let Some(default) = specification.get(1) {
                let mut prepared_specification = specification.clone();
                prepared_specification[1] = self.prepare_compiled_form(default, environment)?;
                prepared[index] = Form::list(prepared_specification, parameter.span);
            }
        }
        Ok(Form::list(prepared, form.span))
    }

    pub(super) fn prepare_local_function_bindings(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(bindings) = &form.kind else {
            return Ok(form.clone());
        };

        let mut prepared_bindings = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                prepared_bindings.push(binding.clone());
                continue;
            };
            let mut prepared_parts = parts.clone();
            if prepared_parts.len() > 1 {
                let parameter_form = parts[1].clone();
                let local =
                    Self::prepare_compiled_lambda_environment(&parameter_form, environment)?;
                prepared_parts[1] = self.prepare_compiled_lambda_list(&parameter_form, &local)?;
                for index in 2..prepared_parts.len() {
                    prepared_parts[index] = self.prepare_compiled_form(&parts[index], &local)?;
                }
            } else {
                for index in 2..prepared_parts.len() {
                    prepared_parts[index] =
                        self.prepare_compiled_form(&parts[index], environment)?;
                }
            }
            prepared_bindings.push(Form::list(prepared_parts, binding.span));
        }
        Ok(Form::list(prepared_bindings, form.span))
    }
}
