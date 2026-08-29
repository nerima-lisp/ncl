use crate::lambda_list_types::{LambdaListAuxiliaryParameter, LambdaListError};
use crate::{Form, FormKind};

use super::names::parse_name;

pub(super) fn parse_auxiliary_parameter(
    form: &Form,
) -> Result<LambdaListAuxiliaryParameter, LambdaListError> {
    match &form.kind {
        FormKind::Atom(_) => {
            let (name, name_escaped) = parse_name(form, "auxiliary parameter")?;
            Ok(LambdaListAuxiliaryParameter {
                name,
                name_escaped,
                init_form: Form::atom("NIL", form.span),
            })
        }
        FormKind::List(items) if (1..=2).contains(&items.len()) => {
            let (name, name_escaped) = parse_name(&items[0], "auxiliary parameter")?;
            let init_form = items
                .get(1)
                .cloned()
                .unwrap_or_else(|| Form::atom("NIL", form.span));
            Ok(LambdaListAuxiliaryParameter {
                name,
                name_escaped,
                init_form,
            })
        }
        FormKind::List(_) => Err(LambdaListError::invalid(
            "auxiliary parameter must contain one or two elements",
            form.span,
        )),
        _ => Err(LambdaListError::expected_symbol(
            "auxiliary parameter",
            form.span,
        )),
    }
}
