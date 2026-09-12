#![allow(clippy::wildcard_imports)]
use super::*;

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

    pub(super) fn prepare_defmethod(
        &self,
        prepared: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        let Some(parameter_index) = prepared
            .iter()
            .enumerate()
            .skip(2)
            .find_map(|(index, form)| matches!(form.kind, FormKind::List(_)).then_some(index))
        else {
            return Ok(());
        };
        let parameter_form = prepared[parameter_index].clone();
        let FormKind::List(parameters) = &parameter_form.kind else {
            return Ok(());
        };
        let mut normalized = parameters.clone();
        for parameter in &mut normalized {
            if atom_name(parameter).is_some_and(|name| normalize_name(name).starts_with('&')) {
                break;
            }
            if let FormKind::List(parts) = &parameter.kind
                && let Some(name) = parts.first()
            {
                *parameter = name.clone();
            }
        }
        // Specializer classes need not exist until the method definition executes.
        let local = Self::prepare_compiled_lambda_environment(
            &Form::list(normalized, parameter_form.span),
            environment,
        )?;
        prepared[parameter_index] = self.prepare_compiled_lambda_list(&parameter_form, &local)?;
        self.prepare_function_body(prepared, parameter_index + 1, &local)
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
        self.prepare_function_body(prepared, body_index, &local)
    }

    fn prepare_function_body(
        &self,
        prepared: &mut [Form],
        body_index: usize,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        let body = prepared.get(body_index..).unwrap_or(&[]);
        let special_names = Self::special_declaration_names(body)?;
        Self::declare_special_names(environment, &special_names);
        self.prepare_tail(prepared, body_index, environment)
    }

    fn prepare_compiled_lambda_environment(
        form: &Form,
        environment: &Environment,
    ) -> Result<Environment, RuntimeError> {
        let _ = form;
        Ok(environment.child())
    }

    pub(super) fn prepare_compiled_lambda_list(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(parameters) = &form.kind else {
            return Ok(form.clone());
        };

        let lambda_list = match Self::parameters(form) {
            Ok(lambda_list) => lambda_list,
            Err(RuntimeError::InvalidForm { .. }) => {
                let mut prepared = parameters.clone();
                let mut default_section = false;
                for (index, parameter) in parameters.iter().enumerate() {
                    if let Some(name) = atom_name(parameter) {
                        match normalize_name(name).as_str() {
                            "&OPTIONAL" | "&KEY" | "&AUX" => default_section = true,
                            "&REST" => default_section = false,
                            _ => {}
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
                        prepared_specification[1] =
                            self.prepare_compiled_form(default, environment)?;
                        prepared[index] = Form::list(prepared_specification, parameter.span);
                    }
                }
                return Ok(Form::list(prepared, form.span));
            }
            Err(error) => return Err(error),
        };

        let mut prepared = parameters.clone();
        let mut section = 0_u8;
        let mut required_index = 0;
        let mut optional_index = 0;
        let mut keyword_index = 0;
        let mut auxiliary_index = 0;
        let define = |environment: &Environment, name: &str, escaped: bool| {
            if escaped {
                environment.define_exact(name, Value::Nil);
            } else {
                environment.define(name, Value::Nil);
            }
        };

        for (index, parameter) in parameters.iter().enumerate() {
            if let Some(name) = atom_name(parameter) {
                match normalize_name(name).as_str() {
                    "&OPTIONAL" => section = 1,
                    "&REST" => section = 2,
                    "&KEY" => section = 4,
                    "&AUX" => section = 5,
                    "&ALLOW-OTHER-KEYS" => {}
                    _ => match section {
                        0 => {
                            if let Some((parameter_name, escaped)) = lambda_list
                                .required
                                .get(required_index)
                                .zip(lambda_list.required_escaped.get(required_index))
                            {
                                define(environment, parameter_name, *escaped);
                                required_index += 1;
                            }
                        }
                        2 => {
                            if let Some(rest) = &lambda_list.rest {
                                define(environment, rest, lambda_list.rest_escaped);
                            }
                            section = 3;
                        }
                        1 => {
                            if let Some(parameter_info) = lambda_list.optional.get(optional_index) {
                                define(
                                    environment,
                                    &parameter_info.name,
                                    parameter_info.name_escaped,
                                );
                                if let Some(name) = &parameter_info.supplied_p {
                                    define(
                                        environment,
                                        name,
                                        parameter_info.supplied_p_escaped.unwrap_or(false),
                                    );
                                }
                                optional_index += 1;
                            }
                        }
                        4 => {
                            if let Some(parameter_info) = lambda_list.keywords.get(keyword_index) {
                                define(
                                    environment,
                                    &parameter_info.name,
                                    parameter_info.name_escaped,
                                );
                                if let Some(name) = &parameter_info.supplied_p {
                                    define(
                                        environment,
                                        name,
                                        parameter_info.supplied_p_escaped.unwrap_or(false),
                                    );
                                }
                                keyword_index += 1;
                            }
                        }
                        5 => {
                            if let Some(parameter_info) = lambda_list.auxiliary.get(auxiliary_index)
                            {
                                define(
                                    environment,
                                    &parameter_info.name,
                                    parameter_info.name_escaped,
                                );
                                auxiliary_index += 1;
                            }
                        }
                        _ => {}
                    },
                }
                continue;
            }

            match section {
                1 => {
                    let Some(parameter_info) = lambda_list.optional.get(optional_index) else {
                        continue;
                    };
                    if let FormKind::List(specification) = &parameter.kind
                        && let Some(default) = specification.get(1)
                    {
                        let mut prepared_specification = specification.clone();
                        prepared_specification[1] =
                            self.prepare_compiled_form(default, environment)?;
                        prepared[index] = Form::list(prepared_specification, parameter.span);
                    }
                    define(
                        environment,
                        &parameter_info.name,
                        parameter_info.name_escaped,
                    );
                    if let Some(name) = &parameter_info.supplied_p {
                        define(
                            environment,
                            name,
                            parameter_info.supplied_p_escaped.unwrap_or(false),
                        );
                    }
                    optional_index += 1;
                }
                4 => {
                    let Some(parameter_info) = lambda_list.keywords.get(keyword_index) else {
                        continue;
                    };
                    if let FormKind::List(specification) = &parameter.kind
                        && let Some(default) = specification.get(1)
                    {
                        let mut prepared_specification = specification.clone();
                        prepared_specification[1] =
                            self.prepare_compiled_form(default, environment)?;
                        prepared[index] = Form::list(prepared_specification, parameter.span);
                    }
                    define(
                        environment,
                        &parameter_info.name,
                        parameter_info.name_escaped,
                    );
                    if let Some(name) = &parameter_info.supplied_p {
                        define(
                            environment,
                            name,
                            parameter_info.supplied_p_escaped.unwrap_or(false),
                        );
                    }
                    keyword_index += 1;
                }
                5 => {
                    let Some(parameter_info) = lambda_list.auxiliary.get(auxiliary_index) else {
                        continue;
                    };
                    if let FormKind::List(specification) = &parameter.kind
                        && let Some(default) = specification.get(1)
                    {
                        let mut prepared_specification = specification.clone();
                        prepared_specification[1] =
                            self.prepare_compiled_form(default, environment)?;
                        prepared[index] = Form::list(prepared_specification, parameter.span);
                    }
                    define(
                        environment,
                        &parameter_info.name,
                        parameter_info.name_escaped,
                    );
                    auxiliary_index += 1;
                }
                _ => {}
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
                self.prepare_function_body(&mut prepared_parts, 2, &local)?;
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
