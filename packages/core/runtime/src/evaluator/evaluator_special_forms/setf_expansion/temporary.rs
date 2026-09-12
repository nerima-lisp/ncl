use std::collections::HashSet;

use super::{Environment, Form, FormKind, Runtime, SetfExpansion, Span, resolved_symbol};

#[derive(Default)]
pub(super) struct SetfTemporaryContext {
    names: HashSet<String>,
    symbol_macros: HashSet<(String, bool)>,
}

impl SetfTemporaryContext {
    pub(super) fn reserve_form(&mut self, form: &Form, environment: &Environment) {
        match &form.kind {
            FormKind::Atom(atom) => {
                let (name, escaped) = resolved_symbol(atom);
                self.names.insert(name.to_ascii_uppercase());
                if self.symbol_macros.insert((name.clone(), escaped)) {
                    let expansion = if escaped {
                        environment.lookup_symbol_macro_exact(&name)
                    } else {
                        environment.lookup_symbol_macro(&name)
                    };
                    if let Some(expansion) = expansion {
                        self.reserve_form(&expansion, environment);
                    }
                }
            }
            FormKind::List(items) | FormKind::Vector(items) => {
                for item in items {
                    self.reserve_form(item, environment);
                }
            }
            FormKind::DottedList { items, tail } => {
                for item in items {
                    self.reserve_form(item, environment);
                }
                self.reserve_form(tail, environment);
            }
            FormKind::Complex { real, imaginary } => {
                self.reserve_form(real, environment);
                self.reserve_form(imaginary, environment);
            }
            FormKind::String(_)
            | FormKind::Character(_)
            | FormKind::Literal(_)
            | FormKind::CircularReference => {}
        }
    }

    pub(super) fn reserve_expansion(
        &mut self,
        expansion: &SetfExpansion,
        environment: &Environment,
    ) {
        for form in expansion
            .temporaries
            .iter()
            .chain(&expansion.values)
            .chain(&expansion.stores)
        {
            self.reserve_form(form, environment);
        }
        self.reserve_form(&expansion.store_form, environment);
        self.reserve_form(&expansion.access_form, environment);
    }
}

impl Runtime {
    pub(super) fn fresh_setf_temporary(
        &self,
        span: Span,
        environment: &Environment,
        context: &mut SetfTemporaryContext,
    ) -> Form {
        loop {
            let counter = self.gensym_counter.get();
            self.gensym_counter.set(counter.wrapping_add(1));
            let name = format!("NCL-SETF-TEMP-{counter}");
            if !context.names.contains(&name)
                && !self.compilation_reserved_names.borrow().contains(&name)
                && !self.dynamic_candidates(&name).iter().any(|candidate| {
                    self.dynamic
                        .borrow()
                        .special_names
                        .contains(candidate.as_str())
                })
                && environment.lookup(&name).is_none()
                && environment.lookup_exact(&name).is_none()
                && environment.lookup_symbol_macro(&name).is_none()
                && environment.lookup_symbol_macro_exact(&name).is_none()
            {
                context.names.insert(name.clone());
                return Form::atom(name, span);
            }
        }
    }
}
