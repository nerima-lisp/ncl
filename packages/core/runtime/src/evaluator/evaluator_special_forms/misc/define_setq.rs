use ncl_syntax::Form;

use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_define(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(Self::arity("define", "two", items.len().saturating_sub(1)));
        }
        let (name, escaped) = Self::variable_name_info(&items[1], "define name must be a symbol")?;
        let value = self.eval_in(&items[2], environment)?;
        self.define_variable_in(&name, escaped, value.clone(), environment);
        Ok(value)
    }

    pub(crate) fn special_setq(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "setq needs variable/value pairs",
                items[0].span,
            ));
        }
        let mut result = Value::Nil;
        for pair in items[1..].as_chunks::<2>().0 {
            let expansion = Self::expand_symbol_macro_form(&pair[0], environment)?;
            let (name, escaped) =
                Self::variable_name_info(&pair[0], "setq target must be a symbol")?;
            result = self.eval_in(&pair[1], environment)?;
            if let Some(place) = expansion {
                self.set_place(&place, result.clone(), environment)?;
            } else {
                self.set_or_define_variable_in(
                    &name,
                    escaped,
                    result.clone(),
                    environment,
                    pair[0].span,
                )?;
            }
        }
        Ok(result)
    }
}
