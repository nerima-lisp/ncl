use crate::lambda_list_types::{LambdaListError, LambdaListOptionalParameter};
use crate::{Form, FormKind};

use super::names::parse_name;

pub(super) fn parse_optional_parameter(
    form: &Form,
) -> Result<LambdaListOptionalParameter, LambdaListError> {
    match &form.kind {
        FormKind::Atom(_) => {
            let (name, name_escaped) = parse_name(form, "optional parameter")?;
            Ok(LambdaListOptionalParameter {
                name,
                name_escaped,
                init_form: Form::atom("NIL", form.span),
                init_form_supplied: false,
                supplied_p: None,
                supplied_p_escaped: None,
            })
        }
        FormKind::List(items) if (1..=3).contains(&items.len()) => {
            let (name, name_escaped) = parse_name(&items[0], "optional parameter")?;
            let init_form = items
                .get(1)
                .cloned()
                .unwrap_or_else(|| Form::atom("NIL", form.span));
            let (supplied_p, supplied_p_escaped) = items
                .get(2)
                .map(|supplied_p| parse_name(supplied_p, "supplied-p parameter"))
                .transpose()?
                .map_or((None, None), |(name, escaped)| (Some(name), Some(escaped)));
            Ok(LambdaListOptionalParameter {
                name,
                name_escaped,
                init_form,
                init_form_supplied: items.get(1).is_some(),
                supplied_p,
                supplied_p_escaped,
            })
        }
        FormKind::List(_) => Err(LambdaListError::invalid(
            "optional parameter must contain one to three elements",
            form.span,
        )),
        _ => Err(LambdaListError::expected_symbol(
            "optional parameter",
            form.span,
        )),
    }
}
