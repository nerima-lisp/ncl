use std::collections::HashSet;

use ncl_syntax::{Form, FormKind};

use crate::environment::normalize_name;
use crate::evaluator::evaluator_state::MacroLambdaListSection;
use crate::evaluator::helpers::atom_name;
use crate::value::MacroLambdaList;
use crate::{Runtime, RuntimeError};

impl Runtime {
    pub(crate) fn macro_parameters(form: &Form) -> Result<MacroLambdaList, RuntimeError> {
        let FormKind::List(parameters) = &form.kind else {
            return Err(Self::invalid("macro parameters must be a list", form.span));
        };

        let mut lambda_list = MacroLambdaList {
            whole: None,
            environment: None,
            required: Vec::new(),
            optional: Vec::new(),
            rest: None,
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            auxiliary: Vec::new(),
        };
        let mut seen = HashSet::new();
        let mut section = MacroLambdaListSection::Required;
        let mut index = 0;

        while index < parameters.len() {
            let parameter = &parameters[index];
            if let Some(name) = atom_name(parameter) {
                let marker = normalize_name(name);
                if let Some(next_index) = Self::handle_macro_marker(
                    &mut lambda_list,
                    &mut section,
                    parameters,
                    index,
                    parameter,
                    &marker,
                    &mut seen,
                )? {
                    index = next_index;
                    continue;
                }
            }

            Self::push_macro_parameter(&mut lambda_list, section, parameter, &mut seen)?;
            index += 1;
        }

        Ok(lambda_list)
    }

    pub(super) fn push_macro_parameter(
        lambda_list: &mut MacroLambdaList,
        section: MacroLambdaListSection,
        parameter: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<(), RuntimeError> {
        if section == MacroLambdaListSection::Rest {
            return Err(Self::invalid(
                "macro rest parameter must be followed by a keyword or auxiliary section",
                parameter.span,
            ));
        }
        match section {
            MacroLambdaListSection::Required => {
                lambda_list
                    .required
                    .push(Self::macro_pattern(parameter, seen)?);
            }
            MacroLambdaListSection::Optional => {
                lambda_list
                    .optional
                    .push(Self::parse_macro_optional_parameter(parameter, seen)?);
            }
            MacroLambdaListSection::Keyword => {
                if lambda_list.allow_other_keys {
                    return Err(Self::invalid(
                        "&allow-other-keys must be the last keyword-list marker",
                        parameter.span,
                    ));
                }
                let specification = Self::parse_macro_keyword_parameter(parameter, seen)?;
                if lambda_list
                    .keywords
                    .iter()
                    .any(|item| item.keyword_name == specification.keyword_name)
                {
                    return Err(Self::invalid(
                        "macro keyword names must be unique",
                        parameter.span,
                    ));
                }
                lambda_list.keywords.push(specification);
            }
            MacroLambdaListSection::Auxiliary => {
                lambda_list
                    .auxiliary
                    .push(Self::parse_macro_auxiliary_parameter(parameter, seen)?);
            }
            MacroLambdaListSection::Rest => unreachable!(),
        }
        Ok(())
    }
}
