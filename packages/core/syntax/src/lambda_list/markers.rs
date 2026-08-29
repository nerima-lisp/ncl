use std::collections::HashSet;

use crate::lambda_list_types::LambdaListError;
use crate::{Form, Span};

use super::names::{marker_name, parse_name};

/// The section of an ordinary lambda-list currently being parsed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LambdaListSection {
    Required,
    Optional,
    Rest,
    Keyword,
    Auxiliary,
}

/// Records `name` as seen, rejecting a lambda-list that binds the same
/// variable name more than once.
pub(super) fn insert_unique(
    names: &mut HashSet<String>,
    name: &str,
    span: Span,
) -> Result<(), LambdaListError> {
    if names.insert(name.to_owned()) {
        Ok(())
    } else {
        Err(LambdaListError::invalid(
            "parameter names must be unique",
            span,
        ))
    }
}

/// The effect of recognizing a lambda-list marker (`&OPTIONAL`, `&REST`, ...).
pub(super) enum MarkerOutcome {
    Optional,
    Rest { name: String, escaped: bool },
    Key,
    AllowOtherKeys,
    Aux,
}

/// Attempts to interpret `parameter` (already known to be a plain symbol
/// named `marker`) as a lambda-list marker.
///
/// Returns `Ok(None)` when `marker` is an ordinary parameter name rather
/// than a recognized marker, in which case the caller should continue
/// parsing `parameter` as a regular section item.
#[expect(clippy::too_many_arguments)]
pub(super) fn recognize_marker(
    marker: &str,
    parameter: &Form,
    parameters: &[Form],
    index: usize,
    section: LambdaListSection,
    rest_is_set: bool,
    has_keyword_section: bool,
    allow_other_keys: bool,
    names: &mut HashSet<String>,
) -> Result<Option<(MarkerOutcome, usize)>, LambdaListError> {
    match marker {
        "&OPTIONAL" => {
            if !matches!(section, LambdaListSection::Required) {
                return Err(LambdaListError::invalid(
                    "&optional may appear only once at the beginning of the lambda-list",
                    parameter.span,
                ));
            }
            Ok(Some((MarkerOutcome::Optional, index + 1)))
        }
        "&REST" => {
            if rest_is_set
                || matches!(
                    section,
                    LambdaListSection::Keyword | LambdaListSection::Auxiliary
                )
            {
                return Err(LambdaListError::invalid(
                    "&rest may appear only once before &key or &aux",
                    parameter.span,
                ));
            }
            let Some(rest_parameter) = parameters.get(index + 1) else {
                return Err(LambdaListError::invalid(
                    "&rest must be followed by one parameter",
                    parameter.span,
                ));
            };
            if marker_name(rest_parameter).is_some_and(|name| name.starts_with('&')) {
                return Err(LambdaListError::invalid(
                    "&rest must be followed by one parameter",
                    rest_parameter.span,
                ));
            }
            let (rest_name, escaped) = parse_name(rest_parameter, "&rest parameter")?;
            insert_unique(names, &rest_name, rest_parameter.span)?;
            Ok(Some((
                MarkerOutcome::Rest {
                    name: rest_name,
                    escaped,
                },
                index + 2,
            )))
        }
        "&KEY" => {
            if has_keyword_section || matches!(section, LambdaListSection::Auxiliary) {
                return Err(LambdaListError::invalid(
                    "&key may appear only once before &aux",
                    parameter.span,
                ));
            }
            Ok(Some((MarkerOutcome::Key, index + 1)))
        }
        "&ALLOW-OTHER-KEYS" => {
            if !has_keyword_section
                || !matches!(section, LambdaListSection::Keyword)
                || allow_other_keys
            {
                return Err(LambdaListError::invalid(
                    "&allow-other-keys requires one &key section and may appear only once",
                    parameter.span,
                ));
            }
            Ok(Some((MarkerOutcome::AllowOtherKeys, index + 1)))
        }
        "&AUX" => {
            if matches!(section, LambdaListSection::Auxiliary) {
                return Err(LambdaListError::invalid(
                    "&aux may appear only once",
                    parameter.span,
                ));
            }
            Ok(Some((MarkerOutcome::Aux, index + 1)))
        }
        _ if marker.starts_with('&') => Err(LambdaListError::invalid(
            format!("unsupported lambda-list marker {marker}"),
            parameter.span,
        )),
        _ => Ok(None),
    }
}
