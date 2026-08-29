use super::{Environment, Form, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_defvar(
        &self,
        items: &[Form],
        environment: &Environment,
        force: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if force { "defparameter" } else { "defvar" };
        if !(items.len() == 2 || items.len() == 3) {
            return Err(Self::arity(
                operator,
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let context = if force {
            "defparameter name must be a symbol"
        } else {
            "defvar name must be a symbol"
        };
        let (name, escaped) = Self::variable_name_info(&items[1], context)?;
        if force
            && if escaped {
                self.is_constant_exact_in(&name)
            } else {
                self.is_constant_in(&name)
            }
        {
            return Err(Self::constant_modification_error(&name, items[1].span));
        }
        if !force {
            let existing = if escaped {
                self.lookup_special_exact(&name)
            } else {
                self.lookup_special(&name)
            };
            if let Some(value) = existing {
                return Ok(value);
            }
        }
        let value = items
            .get(2)
            .map_or(Ok(Value::Nil), |form| self.eval_in(form, environment))?;
        Ok(if escaped {
            self.define_special_value_exact(&name, value, force)
        } else {
            self.define_special_value(&name, value, force)
        })
    }

    pub(crate) fn special_defconstant(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(Self::arity(
                "defconstant",
                "two or three",
                items.len().saturating_sub(1),
            ));
        }
        let (name, escaped) =
            Self::variable_name_info(&items[1], "defconstant name must be a symbol")?;
        if if escaped {
            self.is_constant_exact_in(&name)
        } else {
            self.is_constant_in(&name)
        } {
            return Err(Self::constant_modification_error(&name, items[1].span));
        }
        let value = self.eval_in(&items[2], environment)?;
        Ok(if escaped {
            self.define_constant_value_exact(&name, value)
        } else {
            self.define_constant_value(&name, value)
        })
    }
}
