use std::collections::HashSet;

use ncl_syntax::Form;

use crate::evaluator::evaluator_state::MacroLambdaListSection;
use crate::value::MacroLambdaList;
use crate::{Runtime, RuntimeError};

impl Runtime {
    pub(super) fn handle_macro_marker(
        lambda_list: &mut MacroLambdaList,
        section: &mut MacroLambdaListSection,
        parameters: &[Form],
        index: usize,
        parameter: &Form,
        marker: &str,
        seen: &mut HashSet<String>,
    ) -> Result<Option<usize>, RuntimeError> {
        let next = match marker {
            "&WHOLE" => {
                if index != 0 || lambda_list.whole.is_some() || index + 1 >= parameters.len() {
                    return Err(Self::invalid(
                        "&whole must be the first marker followed by one parameter",
                        parameter.span,
                    ));
                }
                lambda_list.whole = Some(Self::macro_binding_name(&parameters[index + 1], seen)?);
                index + 2
            }
            "&OPTIONAL" => {
                if *section != MacroLambdaListSection::Required {
                    return Err(Self::invalid(
                        "&optional is out of order in macro lambda list",
                        parameter.span,
                    ));
                }
                *section = MacroLambdaListSection::Optional;
                index + 1
            }
            "&REST" | "&BODY" => {
                if lambda_list.rest.is_some()
                    || matches!(
                        section,
                        MacroLambdaListSection::Rest
                            | MacroLambdaListSection::Keyword
                            | MacroLambdaListSection::Auxiliary
                    )
                    || index + 1 >= parameters.len()
                {
                    return Err(Self::invalid(
                        "&rest or &body must be followed by one parameter",
                        parameter.span,
                    ));
                }
                lambda_list.rest = Some(Self::macro_binding_name(&parameters[index + 1], seen)?);
                *section = MacroLambdaListSection::Rest;
                index + 2
            }
            "&KEY" => {
                if lambda_list.has_keyword_section
                    || matches!(
                        section,
                        MacroLambdaListSection::Keyword | MacroLambdaListSection::Auxiliary
                    )
                {
                    return Err(Self::invalid(
                        "&key is out of order or repeated in macro lambda list",
                        parameter.span,
                    ));
                }
                lambda_list.has_keyword_section = true;
                *section = MacroLambdaListSection::Keyword;
                index + 1
            }
            "&ALLOW-OTHER-KEYS" => {
                if *section != MacroLambdaListSection::Keyword || lambda_list.allow_other_keys {
                    return Err(Self::invalid(
                        "&allow-other-keys requires a keyword section",
                        parameter.span,
                    ));
                }
                lambda_list.allow_other_keys = true;
                index + 1
            }
            "&AUX" => {
                if *section == MacroLambdaListSection::Auxiliary {
                    return Err(Self::invalid(
                        "&aux is repeated in macro lambda list",
                        parameter.span,
                    ));
                }
                *section = MacroLambdaListSection::Auxiliary;
                index + 1
            }
            "&ENVIRONMENT" => {
                if lambda_list.environment.is_some() || index + 1 >= parameters.len() {
                    return Err(Self::invalid(
                        "&environment must be followed by one parameter",
                        parameter.span,
                    ));
                }
                lambda_list.environment =
                    Some(Self::macro_binding_name(&parameters[index + 1], seen)?);
                index + 2
            }
            _ if marker.starts_with('&') => {
                return Err(Self::invalid(
                    "unsupported marker in macro lambda list",
                    parameter.span,
                ));
            }
            _ => return Ok(None),
        };
        Ok(Some(next))
    }
}
