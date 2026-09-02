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
        if !matches!(&items[2].kind, FormKind::List(arguments) if arguments.is_empty()) {
            return Err(Self::invalid(
                "deftype currently supports only an empty lambda list",
                items[2].span,
            ));
        }
        let designator = Self::quoted_value(&items[3])?;
        environment.define_type_alias(unqualified_name(&name), designator);
        Ok(Value::symbol(unqualified_name(&name)))
    }
}
