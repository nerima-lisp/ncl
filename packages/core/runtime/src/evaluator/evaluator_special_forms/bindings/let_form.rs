use super::{Environment, Form, FormKind, Runtime, RuntimeError, Value};
use std::collections::HashSet;

impl Runtime {
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
        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let mut special_names = HashSet::new();
        for form in &items[2..] {
            let FormKind::List(declaration) = &form.kind else {
                break;
            };
            if !declaration
                .first()
                .is_some_and(|name| Self::variable_name_info(name, "declare name").is_ok())
            {
                break;
            }
            let Some(FormKind::Atom(name)) = declaration.first().map(|form| &form.kind) else {
                break;
            };
            if !name.eq_ignore_ascii_case("DECLARE") {
                break;
            }
            for spec in &declaration[1..] {
                let FormKind::List(parts) = &spec.kind else {
                    continue;
                };
                if parts.first().is_some_and(|form| {
                    matches!(&form.kind, FormKind::Atom(name) if name.eq_ignore_ascii_case("SPECIAL"))
                }) {
                    for name in &parts[1..] {
                        if let Ok((name, escaped)) =
                            Self::variable_name_info(name, "declare special name")
                        {
                            special_names.insert((name, escaped));
                        }
                    }
                }
            }
        }
        for binding in bindings {
            let FormKind::List(binding_items) = &binding.kind else {
                return Err(Self::invalid("let binding must be a list", binding.span));
            };
            if !(binding_items.len() == 1 || binding_items.len() == 2) {
                return Err(Self::invalid(
                    "let binding needs a name and optional value",
                    binding.span,
                ));
            }
            let (name, escaped) =
                Self::variable_name_info(&binding_items[0], "let binding name must be a symbol")?;
            let value = binding_items.get(1).map_or(Ok(Value::Nil), |form| {
                self.eval_in(form, if sequential { &local } else { environment })
            })?;
            if special_names.contains(&(name.clone(), escaped)) {
                if escaped {
                    self.define_dynamic_exact(&name, value);
                } else {
                    self.define_dynamic(&name, value);
                }
            } else {
                self.define_variable_in(&name, escaped, value, &local);
            }
        }
        self.eval_sequence_values(&items[2..], &local)
    }
}
