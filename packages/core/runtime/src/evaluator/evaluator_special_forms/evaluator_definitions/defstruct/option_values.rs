use super::{Form, OrdinaryLambdaList, Runtime, RuntimeError, is_nil_form, unqualified_name};

impl Runtime {
    pub(super) fn defstruct_name_option(
        option_form: &Form,
        option_items: &[Form],
        default_name: String,
        context: &str,
    ) -> Result<Option<String>, RuntimeError> {
        if option_items.len() > 2 {
            return Err(Self::invalid(
                "defstruct naming options accept at most one name",
                option_form.span,
            ));
        }
        let Some(name_form) = option_items.get(1) else {
            return Ok(Some(default_name));
        };
        if is_nil_form(name_form) {
            return Ok(None);
        }
        let (raw_name, _) = Self::variable_name_info(name_form, context)?;
        Ok(Some(unqualified_name(&raw_name)))
    }

    pub(super) fn defstruct_constructor_option(
        option_form: &Form,
        option_items: &[Form],
        default_name: String,
    ) -> Result<(Option<String>, Option<OrdinaryLambdaList>), RuntimeError> {
        if option_items.len() > 3 {
            return Err(Self::invalid(
                "defstruct :constructor accepts at most a name and a lambda list",
                option_form.span,
            ));
        }
        let constructor_name = match option_items.get(1) {
            None => Some(default_name),
            Some(name_form) if is_nil_form(name_form) => None,
            Some(name_form) => {
                let (raw_name, _) = Self::variable_name_info(
                    name_form,
                    "defstruct :constructor must name a symbol or NIL",
                )?;
                Some(unqualified_name(&raw_name))
            }
        };
        let constructor_lambda_list = option_items
            .get(2)
            .map(|lambda_list_form| {
                if constructor_name.is_none() {
                    return Err(Self::invalid(
                        "defstruct :constructor NIL cannot have a lambda list",
                        lambda_list_form.span,
                    ));
                }
                Self::parameters(lambda_list_form)
            })
            .transpose()?;
        Ok((constructor_name, constructor_lambda_list))
    }
}
