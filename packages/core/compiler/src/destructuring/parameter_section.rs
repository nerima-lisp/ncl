#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    /// Compiles a lambda-list parameter that is not a marker (`&WHOLE`,
    /// `&OPTIONAL`, ...) into whichever section is currently active.
    pub(super) fn compile_destructuring_regular_parameter(
        &mut self,
        parameter: &Form,
        section: DestructureLambdaListSection,
        lambda_list: &mut DestructureLambdaList,
        seen: &mut HashSet<String>,
    ) -> Result<(), CompileError> {
        if section == DestructureLambdaListSection::Rest {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring rest parameter must be followed by a keyword or auxiliary section"
                        .to_string(),
                },
                parameter.span,
            ));
        }
        match section {
            DestructureLambdaListSection::Required => lambda_list
                .required
                .push(Self::compile_destructuring_pattern(parameter, seen)?),
            DestructureLambdaListSection::Optional => lambda_list
                .optional
                .push(self.compile_destructuring_optional_parameter(parameter, seen)?),
            DestructureLambdaListSection::Keyword => {
                if lambda_list.allow_other_keys {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "&allow-other-keys must be the last keyword-list marker"
                                .to_string(),
                        },
                        parameter.span,
                    ));
                }
                let specification =
                    self.compile_destructuring_keyword_parameter(parameter, seen)?;
                if lambda_list
                    .keywords
                    .iter()
                    .any(|item| item.keyword_name == specification.keyword_name)
                {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "destructuring keyword names must be unique".to_string(),
                        },
                        parameter.span,
                    ));
                }
                lambda_list.keywords.push(specification);
            }
            DestructureLambdaListSection::Auxiliary => lambda_list
                .auxiliary
                .push(self.compile_destructuring_auxiliary_parameter(parameter, seen)?),
            DestructureLambdaListSection::Rest => unreachable!(
                "the early return above already rejected DestructureLambdaListSection::Rest"
            ),
        }
        Ok(())
    }
}
