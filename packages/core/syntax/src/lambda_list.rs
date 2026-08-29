use std::collections::HashSet;

use crate::lambda_list_types::{LambdaListError, LambdaListErrorKind, OrdinaryLambdaList};
use crate::{Form, FormKind};

mod auxiliary;
mod keyword;
mod markers;
mod names;
mod optional;
use auxiliary::parse_auxiliary_parameter;
use keyword::parse_keyword_parameter;
use markers::{LambdaListSection, MarkerOutcome, insert_unique, recognize_marker};
pub use names::normalize_name;
use names::{marker_name, parse_name};
use optional::parse_optional_parameter;
/// Parse the Common Lisp ordinary lambda-list subset supported by ncl.
///
/// The accepted grammar is:
///
/// ```text
/// lambda-list ::= required* [&OPTIONAL optional-spec*] [&REST name]
///                [&KEY keyword-spec* [&ALLOW-OTHER-KEYS]] [&AUX auxiliary-spec*]
/// optional-spec ::= name | (name init-form [supplied-p])
/// keyword-spec ::= name | (name init-form [supplied-p])
///                | ((keyword-name name) init-form [supplied-p])
/// auxiliary-spec ::= name | (name init-form)
/// ```
///
/// Other lambda-list markers are rejected so that the compiler and the
/// interpreter fail consistently instead of silently treating one as a name.
///
/// # Errors
///
/// Returns [`LambdaListError`] when the form does not follow the grammar.
// One state machine over ~10 mutable per-section accumulators; splitting it
// only moves the same argument list into a helper, it doesn't shorten it.
#[allow(clippy::too_many_lines)]
pub fn parse_ordinary_lambda_list(form: &Form) -> Result<OrdinaryLambdaList, LambdaListError> {
    let parameters: &[Form] = match &form.kind {
        FormKind::List(parameters) => parameters,
        // Runtime values represent NIL as an atom; needed when a quoted
        // lambda form is reconstructed for EVAL/COMPILE.
        FormKind::Atom(name) if name == "NIL" => &[],
        _ => {
            return Err(LambdaListError {
                kind: LambdaListErrorKind::ExpectedList,
                span: form.span,
            });
        }
    };

    let mut required = Vec::new();
    let mut required_escaped = Vec::new();
    let mut optional = Vec::new();
    let mut rest = None;
    let mut rest_escaped = false;
    let mut keywords = Vec::new();
    let mut has_keyword_section = false;
    let mut allow_other_keys = false;
    let mut auxiliary = Vec::new();
    let mut names = HashSet::new();
    let mut keyword_names = HashSet::new();
    let mut section = LambdaListSection::Required;
    let mut index = 0;

    while index < parameters.len() {
        let parameter = &parameters[index];
        if let Some(marker) = marker_name(parameter)
            && let Some((outcome, next_index)) = recognize_marker(
                &marker,
                parameter,
                parameters,
                index,
                section,
                rest.is_some(),
                has_keyword_section,
                allow_other_keys,
                &mut names,
            )?
        {
            match outcome {
                MarkerOutcome::Optional => section = LambdaListSection::Optional,
                MarkerOutcome::Rest { name, escaped } => {
                    rest = Some(name);
                    rest_escaped = escaped;
                    section = LambdaListSection::Rest;
                }
                MarkerOutcome::Key => {
                    has_keyword_section = true;
                    section = LambdaListSection::Keyword;
                }
                MarkerOutcome::AllowOtherKeys => allow_other_keys = true,
                MarkerOutcome::Aux => section = LambdaListSection::Auxiliary,
            }
            index = next_index;
            continue;
        }

        match section {
            LambdaListSection::Required => {
                let (name, escaped) = parse_name(parameter, "parameter")?;
                insert_unique(&mut names, &name, parameter.span)?;
                required.push(name);
                required_escaped.push(escaped);
            }
            LambdaListSection::Optional => {
                let specification = parse_optional_parameter(parameter)?;
                insert_unique(&mut names, &specification.name, parameter.span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    insert_unique(&mut names, supplied_p, parameter.span)?;
                }
                optional.push(specification);
            }
            LambdaListSection::Rest => {
                return Err(LambdaListError::invalid(
                    "&rest must be followed by &key, &aux, or end of lambda-list",
                    parameter.span,
                ));
            }
            LambdaListSection::Keyword => {
                if allow_other_keys {
                    return Err(LambdaListError::invalid(
                        "&allow-other-keys must be the last item in the &key section",
                        parameter.span,
                    ));
                }
                let specification = parse_keyword_parameter(parameter)?;
                if !keyword_names.insert(specification.keyword_name.clone()) {
                    return Err(LambdaListError::invalid(
                        "keyword names must be unique",
                        parameter.span,
                    ));
                }
                insert_unique(&mut names, &specification.name, parameter.span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    insert_unique(&mut names, supplied_p, parameter.span)?;
                }
                keywords.push(specification);
            }
            LambdaListSection::Auxiliary => {
                let specification = parse_auxiliary_parameter(parameter)?;
                insert_unique(&mut names, &specification.name, parameter.span)?;
                auxiliary.push(specification);
            }
        }
        index += 1;
    }

    Ok(OrdinaryLambdaList {
        required,
        required_escaped,
        optional,
        rest,
        rest_escaped,
        keywords,
        has_keyword_section,
        allow_other_keys,
        auxiliary,
    })
}
