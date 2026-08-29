#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    /// Applies one `&`-prefixed lambda-list marker (`marker` must already be
    /// known to start with `&`) and returns the index to resume scanning
    /// from.
    // A single match over the closed set of CL lambda-list markers reads
    // more clearly as one dispatch than as eight near-identical helpers
    // each re-threading the same six parameters.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn compile_destructuring_apply_marker(
        marker: &str,
        parameter: &Form,
        parameters: &[Form],
        index: usize,
        lambda_list: &mut DestructureLambdaList,
        seen: &mut HashSet<String>,
        section: &mut DestructureLambdaListSection,
    ) -> Result<usize, CompileError> {
        match marker {
            "&WHOLE" => {
                if index != 0 || lambda_list.whole.is_some() || index + 1 >= parameters.len() {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "&whole must be the first marker followed by one parameter"
                                .to_string(),
                        },
                        parameter.span,
                    ));
                }
                lambda_list.whole = Some(Self::compile_destructuring_binding_name(
                    &parameters[index + 1],
                    seen,
                    "destructuring whole parameter name",
                )?);
                Ok(index + 2)
            }
            "&OPTIONAL" => {
                if *section != DestructureLambdaListSection::Required {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "&optional is out of order in destructuring lambda list"
                                .to_string(),
                        },
                        parameter.span,
                    ));
                }
                *section = DestructureLambdaListSection::Optional;
                Ok(index + 1)
            }
            "&REST" | "&BODY" => {
                if lambda_list.rest.is_some()
                    || matches!(
                        section,
                        DestructureLambdaListSection::Rest
                            | DestructureLambdaListSection::Keyword
                            | DestructureLambdaListSection::Auxiliary
                    )
                    || index + 1 >= parameters.len()
                {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "&rest or &body must be followed by one parameter".to_string(),
                        },
                        parameter.span,
                    ));
                }
                lambda_list.rest = Some(Self::compile_destructuring_binding_name(
                    &parameters[index + 1],
                    seen,
                    "destructuring rest parameter name",
                )?);
                *section = DestructureLambdaListSection::Rest;
                Ok(index + 2)
            }
            "&KEY" => {
                if lambda_list.has_keyword_section
                    || matches!(
                        section,
                        DestructureLambdaListSection::Keyword
                            | DestructureLambdaListSection::Auxiliary
                    )
                {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message:
                                "&key is out of order or repeated in destructuring lambda list"
                                    .to_string(),
                        },
                        parameter.span,
                    ));
                }
                lambda_list.has_keyword_section = true;
                *section = DestructureLambdaListSection::Keyword;
                Ok(index + 1)
            }
            "&ALLOW-OTHER-KEYS" => {
                if *section != DestructureLambdaListSection::Keyword || lambda_list.allow_other_keys
                {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "&allow-other-keys requires a keyword section".to_string(),
                        },
                        parameter.span,
                    ));
                }
                lambda_list.allow_other_keys = true;
                Ok(index + 1)
            }
            "&AUX" => {
                if *section == DestructureLambdaListSection::Auxiliary {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "&aux is repeated in destructuring lambda list".to_string(),
                        },
                        parameter.span,
                    ));
                }
                *section = DestructureLambdaListSection::Auxiliary;
                Ok(index + 1)
            }
            "&ENVIRONMENT" => Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "&environment is not supported in destructuring-bind".to_string(),
                },
                parameter.span,
            )),
            _ => Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "unsupported marker in destructuring lambda list".to_string(),
                },
                parameter.span,
            )),
        }
    }
}
