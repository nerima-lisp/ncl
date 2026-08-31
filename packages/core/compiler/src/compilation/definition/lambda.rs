#![allow(clippy::wildcard_imports)]
use super::super::*;

impl CompileState {
    pub(super) fn lambda_parameter_names(
        lambda_list: &OrdinaryLambdaList,
    ) -> HashSet<(String, bool)> {
        let mut names: HashSet<(String, bool)> = lambda_list
            .required
            .iter()
            .zip(&lambda_list.required_escaped)
            .map(|(name, escaped)| (name.clone(), *escaped))
            .collect();
        names.extend(lambda_list.optional.iter().flat_map(|parameter| {
            std::iter::once((parameter.name.clone(), parameter.name_escaped)).chain(
                parameter
                    .supplied_p
                    .as_ref()
                    .map(|name| (name.clone(), parameter.supplied_p_escaped.unwrap_or(false))),
            )
        }));
        names.extend(lambda_list.keywords.iter().flat_map(|parameter| {
            std::iter::once((parameter.name.clone(), parameter.name_escaped)).chain(
                parameter
                    .supplied_p
                    .as_ref()
                    .map(|name| (name.clone(), parameter.supplied_p_escaped.unwrap_or(false))),
            )
        }));
        if let Some(name) = &lambda_list.rest {
            names.insert((name.clone(), lambda_list.rest_escaped));
        }
        names.extend(
            lambda_list
                .auxiliary
                .iter()
                .map(|parameter| (parameter.name.clone(), parameter.name_escaped)),
        );
        names
    }

    pub(super) fn emit_special_parameter_declarations(
        &mut self,
        function: FunctionId,
        body: &[Form],
        parameter_names: &HashSet<(String, bool)>,
    ) -> Result<(), CompileError> {
        for declaration in body
            .iter()
            .take_while(|form| matches!(form.kind, FormKind::List(_)))
        {
            let FormKind::List(parts) = &declaration.kind else {
                continue;
            };
            if parts
                .first()
                .and_then(|form| Self::symbol_name_info(form, "declaration operator").ok())
                .is_none_or(|(name, _)| !name.eq_ignore_ascii_case("DECLARE"))
            {
                continue;
            }
            for spec in parts.iter().skip(1) {
                let FormKind::List(spec_parts) = &spec.kind else {
                    continue;
                };
                if spec_parts
                    .first()
                    .and_then(|form| Self::symbol_name_info(form, "declaration type").ok())
                    .is_none_or(|(name, _)| !name.eq_ignore_ascii_case("SPECIAL"))
                {
                    continue;
                }
                for name_form in spec_parts.iter().skip(1) {
                    let Ok((name, escaped)) =
                        Self::symbol_name_info(name_form, "special declaration name")
                    else {
                        continue;
                    };
                    if parameter_names.contains(&(name.clone(), escaped)) {
                        self.emit(
                            function,
                            if escaped {
                                Instruction::LoadExact(name.clone())
                            } else {
                                Instruction::Load(name.clone())
                            },
                            name_form.span,
                        )?;
                        self.emit(
                            function,
                            if escaped {
                                Instruction::DefineSpecialExact { name, force: true }
                            } else {
                                Instruction::DefineSpecial { name, force: true }
                            },
                            name_form.span,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn compile_lambda(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "lambda needs parameters and a body".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let lambda_list = Self::parameters(&items[1])?;
        let child = self.reserve_function_with_rest(
            None,
            lambda_list.required.clone(),
            lambda_list.required_escaped.clone(),
            lambda_list.rest.clone(),
            lambda_list.rest_escaped,
        );
        self.functions[child].optional = self.compile_optional_parameters(&lambda_list.optional)?;
        self.functions[child].keywords = self.compile_keyword_parameters(&lambda_list.keywords)?;
        self.functions[child].has_keyword_section = lambda_list.has_keyword_section;
        self.functions[child].allow_other_keys = lambda_list.allow_other_keys;
        self.functions[child].auxiliary =
            self.compile_auxiliary_parameters(&lambda_list.auxiliary)?;
        let body = items.get(2..).unwrap_or(&[]);
        self.emit_special_parameter_declarations(
            child,
            body,
            &Self::lambda_parameter_names(&lambda_list),
        )?;
        self.compile_sequence(child, body)?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(function, Instruction::MakeClosure(child), span)?;
        Ok(())
    }
}
