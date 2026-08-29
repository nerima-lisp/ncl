use super::{DoBinding, Environment, Form, FormKind, HashSet, Runtime, RuntimeError, Value};

impl Runtime {
    pub(super) fn initialize_do_bindings(
        &self,
        bindings: &[DoBinding],
        sequential: bool,
        local: &Environment,
        block_environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if sequential {
            for (name, escaped, init, _) in bindings {
                let value = init
                    .as_ref()
                    .map_or(Ok(Value::Nil), |form| self.eval_in(form, local))?;
                self.define_variable_in(name, *escaped, value, local);
            }
        } else {
            let mut values = Vec::with_capacity(bindings.len());
            for (_, _, init, _) in bindings {
                values.push(
                    init.as_ref()
                        .map_or(Ok(Value::Nil), |form| self.eval_in(form, block_environment))?,
                );
            }
            for ((name, escaped, _, _), value) in bindings.iter().zip(values) {
                self.define_variable_in(name, *escaped, value, local);
            }
        }
        Ok(())
    }

    pub(super) fn advance_do_bindings(
        &self,
        bindings: &[DoBinding],
        sequential: bool,
        local: &Environment,
    ) -> Result<(), RuntimeError> {
        if sequential {
            for (name, escaped, _, step) in bindings {
                if let Some(step) = step {
                    let value = self.eval_in(step, local)?;
                    self.set_variable_in(name, *escaped, value, local);
                }
            }
        } else {
            let mut values = Vec::with_capacity(bindings.len());
            for (_, _, _, step) in bindings {
                values.push(match step {
                    Some(step) => Some(self.eval_in(step, local)?),
                    None => None,
                });
            }
            for ((name, escaped, _, _), value) in bindings.iter().zip(values) {
                if let Some(value) = value {
                    self.set_variable_in(name, *escaped, value, local);
                }
            }
        }
        Ok(())
    }

    pub(super) fn parse_do_bindings(
        binding_forms: &[Form],
    ) -> Result<Vec<DoBinding>, RuntimeError> {
        let mut names = HashSet::new();
        let mut bindings = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let FormKind::List(parts) = &binding.kind else {
                return Err(Self::invalid("do binding must be a list", binding.span));
            };
            if !(1..=3).contains(&parts.len()) {
                return Err(Self::invalid(
                    "do binding needs a name, optional init, and optional step",
                    binding.span,
                ));
            }
            let (name, escaped) =
                Self::variable_name_info(&parts[0], "do binding name must be a symbol")?;
            let key = if escaped {
                format!("\0{name}")
            } else {
                name.clone()
            };
            if !names.insert(key) {
                return Err(Self::invalid(
                    "do binding names must be unique",
                    parts[0].span,
                ));
            }
            bindings.push((name, escaped, parts.get(1).cloned(), parts.get(2).cloned()));
        }
        Ok(bindings)
    }
}
