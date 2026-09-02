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
            let (parameter, _) = Self::variable_name_info(argument, "deftype parameters must be symbols")?;
            if parameter.starts_with('&') {
                return Err(Self::invalid("deftype supports only required parameters", argument.span));
            }
            let parameter = unqualified_name(&parameter);
            parameters.push(crate::environment::intern_name(&parameter));
        }
        let designator = Self::quoted_value(&items[3])?;
        environment.define_type_alias(unqualified_name(&name), parameters, designator);
        Ok(Value::symbol(unqualified_name(&name)))
    }
}
