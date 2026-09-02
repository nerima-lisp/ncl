use super::*;

impl Runtime {
    pub(crate) fn special_deftype(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 4 {
            return Err(Self::arity("deftype", "three", items.len().saturating_sub(1)));
        }
        let (name, _) = Self::variable_name_info(&items[1], "deftype name must be a symbol")?;
        let arguments = match &items[2].kind {
            FormKind::List(arguments) => arguments,
            _ => return Err(Self::invalid("deftype lambda list must be a proper list", items[2].span)),
        };
        let mut parameters = Vec::with_capacity(arguments.len());
        for argument in arguments {
            if let Some(marker) = atom_name(argument) {
                if marker.eq_ignore_ascii_case("&optional") {
                    break;
                }
            }
            let (parameter, _) = Self::variable_name_info(argument, "deftype parameters must be symbols")?;
            let parameter = unqualified_name(&parameter);
            if parameter.starts_with('&') {
                break;
            }
            parameters.push(crate::environment::intern_name(&parameter));
        }
        let marker = parameters.len();
        let mut optional_parameters = Vec::new();
        if marker < arguments.len() {
            if !atom_name(&arguments[marker]).is_some_and(|name| name.eq_ignore_ascii_case("&optional")) {
                return Err(Self::invalid("invalid deftype lambda list", arguments[marker].span));
            }
            for argument in arguments.iter().skip(marker + 1) {
                let (parameter, default) = match &argument.kind {
                    FormKind::Atom(_) => (Self::variable_name_info(argument, "deftype parameter")?.0, Value::Nil),
                    FormKind::List(items) if (1..=2).contains(&items.len()) => {
                        (Self::variable_name_info(&items[0], "deftype parameter")?.0,
                         items.get(1).map(Self::quoted_value).transpose()?.unwrap_or(Value::Nil))
                    }
                    _ => return Err(Self::invalid("invalid deftype optional parameter", argument.span)),
                };
                optional_parameters.push((crate::environment::intern_name(&unqualified_name(&parameter)), default));
            }
        }
        let designator = Self::quoted_value(&items[3])?;
        environment.define_type_alias(unqualified_name(&name), parameters, optional_parameters, designator);
        Ok(Value::symbol(unqualified_name(&name)))
    }
}
