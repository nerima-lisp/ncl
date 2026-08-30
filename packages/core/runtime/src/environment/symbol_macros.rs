use ncl_syntax::Form;

use crate::environment::{Environment, intern_exact_name, intern_name};

impl Environment {
    pub(crate) fn define_symbol_macro(&self, name: impl AsRef<str>, expansion: Form) {
        let key = intern_name(name.as_ref());
        self.0.borrow_mut().symbol_macros.insert(key, expansion);
    }

    pub(crate) fn define_symbol_macro_exact(&self, name: impl AsRef<str>, expansion: Form) {
        self.0
            .borrow_mut()
            .exact_symbol_macros
            .insert(intern_exact_name(name.as_ref()), expansion);
    }

    pub(crate) fn lookup_symbol_macro(&self, name: &str) -> Option<Form> {
        let key = intern_name(name);
        let (expansion, shadowed, parent) = {
            let frame = self.0.borrow();
            (
                frame.symbol_macros.get(&key).cloned(),
                frame.values.contains_key(&key),
                frame.parent.clone(),
            )
        };
        if shadowed {
            None
        } else {
            expansion
                .or_else(|| parent.and_then(|environment| environment.lookup_symbol_macro(name)))
        }
    }

    pub(crate) fn lookup_symbol_macro_exact(&self, name: &str) -> Option<Form> {
        let key = intern_exact_name(name);
        let (expansion, shadowed, parent) = {
            let frame = self.0.borrow();
            (
                frame.exact_symbol_macros.get(&key).cloned(),
                frame.exact_values.contains_key(&key),
                frame.parent.clone(),
            )
        };
        if shadowed {
            None
        } else {
            expansion.or_else(|| {
                parent.and_then(|environment| environment.lookup_symbol_macro_exact(name))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Value;
    use crate::environment::Environment;

    #[test]
    fn symbol_macros_shadow_by_lexical_bindings_and_support_exact_lookup() {
        let root = Environment::new();
        let child = root.child();
        let mut forms = match ncl_syntax::read("replacement") {
            Ok(forms) => forms,
            Err(error) => panic!("test form should parse: {error}"),
        };
        let form = forms.remove(0);

        root.define_symbol_macro("when", form.clone());
        assert!(child.lookup_symbol_macro("WHEN").is_some());
        child.define("when", Value::Nil);
        assert!(child.lookup_symbol_macro("when").is_none());
        assert!(child.lookup_symbol_macro_exact("when").is_none());
        child.define_symbol_macro_exact("when", form);
        assert!(child.lookup_symbol_macro_exact("when").is_some());
    }
}
