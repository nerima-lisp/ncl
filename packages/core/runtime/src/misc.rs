#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn special_macroexpand(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity(
                "macroexpand",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = Self::form_from_value(&value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let (expanded, expanded_p) = self.expand_macros_with_flag(form, &expansion_environment)?;
        Ok(Value::values(vec![
            Self::quoted_value(&expanded)?,
            Value::boolean(expanded_p),
        ]))
    }

    pub(super) fn macroexpansion_environment(
        &self,
        value: Value,
        span: Span,
    ) -> Result<Environment, RuntimeError> {
        match value {
            Value::Nil | Value::Boolean(false) => Ok(self.global.clone()),
            Value::Environment(environment) => Ok(environment),
            _ => Err(Self::invalid(
                "macro expansion environment must be an environment",
                span,
            )),
        }
    }

    pub(super) fn special_define(
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

    pub(super) fn special_setq(
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

    pub(super) fn special_psetq(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "psetq needs variable/value pairs",
                items[0].span,
            ));
        }
        let mut names = Vec::with_capacity((items.len() - 1) / 2);
        for pair in items[1..].as_chunks::<2>().0 {
            let expansion = Self::expand_symbol_macro_form(&pair[0], environment)?;
            names.push((
                Self::variable_name_info(&pair[0], "psetq target must be a symbol")?,
                expansion,
            ));
        }
        let values = items[1..]
            .as_chunks::<2>().0
            .iter()
            .map(|pair| {
                self.eval_values_in(&pair[1], environment)
                    .map(|value| value.primary_value())
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (((name, escaped), expansion), value) in names.iter().zip(values) {
            if let Some(place) = expansion {
                self.set_place(place, value, environment)?;
            } else {
                self.set_or_define_variable_in(name, *escaped, value, environment, items[0].span)?;
            }
        }
        Ok(Value::Nil)
    }

    pub(super) fn special_multiple_value_setq(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(Self::arity(
                "multiple-value-setq",
                "two",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(variable_forms) = &items[1].kind else {
            return Err(Self::invalid(
                "multiple-value-setq variables must be a list",
                items[1].span,
            ));
        };
        let names = variable_forms
            .iter()
            .map(|form| {
                Ok::<_, RuntimeError>((
                    Self::variable_name_info(
                        form,
                        "multiple-value-setq variable must be a symbol",
                    )?,
                    Self::expand_symbol_macro_form(form, environment)?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let source = self.eval_values_in(&items[2], environment)?;
        let values = source.multiple_values();
        for (index, ((name, escaped), expansion)) in names.iter().enumerate() {
            let value = values.get(index).cloned().unwrap_or(Value::Nil);
            if let Some(place) = expansion {
                self.set_place(place, value, environment)?;
            } else {
                self.set_or_define_variable_in(name, *escaped, value, environment, items[0].span)?;
            }
        }
        Ok(source.primary_value())
    }
}
