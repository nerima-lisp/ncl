use super::{Environment, Form, FormKind, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_declaration_names(
        forms: &[Form],
    ) -> Result<Vec<(String, bool)>, RuntimeError> {
        let is_declaration_form = |form: &Form| {
            let FormKind::List(items) = &form.kind else {
                return false;
            };
            let Some(operator) = items.first() else {
                return false;
            };
            Self::variable_name_info(operator, "declaration operator")
                .ok()
                .is_some_and(|(name, _)| name == "DECLARE")
        };
        let mut names = Vec::new();
        let mut index = 0;
        let mut documentation_seen = false;
        while let Some(form) = forms.get(index) {
            if !documentation_seen
                && Self::form_string(form).is_some()
                && forms
                    .get(index + 1)
                    .is_some_and(|next| is_declaration_form(next))
            {
                documentation_seen = true;
                index += 1;
                continue;
            }
            let FormKind::List(items) = &form.kind else {
                break;
            };
            let Some(operator) = items.first() else {
                break;
            };
            let Ok((operator_name, _)) = Self::variable_name_info(operator, "declaration operator")
            else {
                break;
            };
            if operator_name != "DECLARE" {
                break;
            }
            for declaration in items.iter().skip(1) {
                let FormKind::List(specification) = &declaration.kind else {
                    continue;
                };
                let Some(kind) = specification.first() else {
                    continue;
                };
                let Ok((kind_name, _)) = Self::variable_name_info(kind, "declaration name") else {
                    continue;
                };
                if kind_name != "SPECIAL" {
                    continue;
                }
                for name in specification.iter().skip(1) {
                    names.push(Self::variable_name_info(name, "special declaration name")?);
                }
            }
            index += 1;
        }
        Ok(names)
    }

    pub(crate) fn declares_special(names: &[(String, bool)], name: &str, escaped: bool) -> bool {
        names.iter().any(|(declared_name, declared_escaped)| {
            *declared_escaped == escaped && declared_name == name
        })
    }

    pub(crate) fn declare_special_names(environment: &Environment, names: &[(String, bool)]) {
        for (name, escaped) in names {
            if *escaped {
                environment.declare_special_exact(name);
            } else {
                environment.declare_special(name);
            }
        }
    }

    pub(crate) fn special_let(
        &self,
        items: &[Form],
        environment: &Environment,
        sequential: bool,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                if sequential { "let*" } else { "let" },
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(Self::invalid("let bindings must be a list", items[1].span));
        };
        let special_names = Self::special_declaration_names(items.get(2..).unwrap_or(&[]))?;
        let mut local = if sequential {
            environment.clone()
        } else {
            environment.child()
        };
        let _dynamic_guard = self.dynamic_guard();
        let mut pending = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let (name_form, initializer) = if let FormKind::List(binding_items) = &binding.kind {
                if !(binding_items.len() == 1 || binding_items.len() == 2) {
                    return Err(Self::invalid(
                        "let binding needs a name and optional value",
                        binding.span,
                    ));
                }
                (&binding_items[0], binding_items.get(1))
            } else {
                (binding, None)
            };
            let (name, escaped) =
                Self::variable_name_info(name_form, "let binding name must be a symbol")?;
            let value = initializer.map_or(Ok(Value::Nil), |form| {
                self.eval_in(form, if sequential { &local } else { environment })
            })?;
            pending.push((name, escaped, value));
            if sequential {
                let (name, escaped, value) = pending.pop().unwrap_or_else(|| unreachable!());
                local = local.child();
                if Self::declares_special(&special_names, &name, escaped) {
                    Self::declare_special_names(&local, &[(name.clone(), escaped)]);
                }
                self.define_variable_in(&name, escaped, value, &local);
            }
        }
        if !sequential {
            for (name, escaped) in &special_names {
                if pending.iter().any(|(binding_name, binding_escaped, _)| {
                    binding_name == name && binding_escaped == escaped
                }) {
                    Self::declare_special_names(&local, &[(name.clone(), *escaped)]);
                }
            }
            for (name, escaped, value) in pending {
                self.define_variable_in(&name, escaped, value, &local);
            }
        }
        Self::declare_special_names(&local, &special_names);
        self.eval_sequence_values(&items[2..], &local)
    }
}
