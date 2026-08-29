#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn special_if(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(Self::arity(
                "if",
                "two or three",
                items.len().saturating_sub(1),
            ));
        }
        let condition = self.eval_in(&items[1], environment)?;
        if condition.is_truthy() {
            self.eval_values_in(&items[2], environment)
        } else {
            items.get(3).map_or(Ok(Value::Nil), |form| {
                self.eval_values_in(form, environment)
            })
        }
    }

    pub(crate) fn special_progn(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_sequence_values(forms, environment)
    }

    pub(crate) fn special_prog1(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "prog1",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let result = self.eval_values_in(&items[1], environment)?;
        self.eval_sequence_values(&items[2..], environment)?;
        Ok(result)
    }

    pub(crate) fn special_prog2(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "prog2",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        self.eval_values_in(&items[1], environment)?;
        let result = self.eval_values_in(&items[2], environment)?;
        self.eval_sequence_values(&items[3..], environment)?;
        Ok(result)
    }

    pub(crate) fn special_prog(
        &self,
        items: &[Form],
        environment: &Environment,
        sequential: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if sequential { "prog*" } else { "prog" };
        if items.len() < 2 {
            return Err(Self::arity(
                operator,
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(binding_forms) = &items[1].kind else {
            return Err(Self::invalid("prog bindings must be a list", items[1].span));
        };
        let mut names = HashSet::new();
        let mut bindings = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let (name_form, init) = match &binding.kind {
                FormKind::Atom(_) => (binding, None),
                FormKind::List(parts) => {
                    if !(1..=2).contains(&parts.len()) {
                        return Err(Self::invalid(
                            "prog binding needs a name and optional value",
                            binding.span,
                        ));
                    }
                    let Some(name_form) = parts.first() else {
                        return Err(Self::invalid("prog binding needs a name", binding.span));
                    };
                    (name_form, parts.get(1).cloned())
                }
                _ => {
                    return Err(Self::invalid(
                        "prog binding must be a symbol or list",
                        binding.span,
                    ));
                }
            };
            let (name, escaped) =
                Self::variable_name_info(name_form, "prog binding name must be a symbol")?;
            let key = if escaped {
                format!("\0{name}")
            } else {
                name.clone()
            };
            if !names.insert(key) {
                return Err(Self::invalid(
                    "prog binding names must be unique",
                    name_form.span,
                ));
            }
            bindings.push((name, escaped, init));
        }
        let target = self.fresh_block_target();
        let block_environment = environment.child();
        block_environment.define_block("NIL", target);
        let local = block_environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let execute = || -> Result<Value, RuntimeError> {
            if sequential {
                for (name, escaped, init) in &bindings {
                    let value = init
                        .as_ref()
                        .map_or(Ok(Value::Nil), |form| self.eval_in(form, &local))?;
                    self.define_variable_in(name, *escaped, value, &local);
                }
            } else {
                let mut values = Vec::with_capacity(bindings.len());
                for (_, _, init) in &bindings {
                    values.push(init.as_ref().map_or(Ok(Value::Nil), |form| {
                        self.eval_in(form, &block_environment)
                    })?);
                }
                for ((name, escaped, _), value) in bindings.iter().zip(values) {
                    self.define_variable_in(name, *escaped, value, &local);
                }
            }
            self.eval_tagbody_forms(&items[2..], &local)?;
            Ok(Value::Nil)
        };
        match execute() {
            Ok(value) => Ok(value),
            Err(RuntimeError::ReturnFrom {
                target: Some(return_target),
                value,
                ..
            }) if return_target == target => Ok(value.into_value()),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn special_values(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let values = items[1..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Value::values(values))
    }

    pub(crate) fn special_multiple_value_list(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(Self::arity(
                "multiple-value-list",
                "one",
                items.len().saturating_sub(1),
            ));
        }
        let values = self
            .eval_values_in(&items[1], environment)?
            .multiple_values();
        Ok(Value::list(values))
    }
}
