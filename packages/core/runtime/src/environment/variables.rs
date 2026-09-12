use crate::Value;
use crate::environment::{Environment, intern_exact_name, intern_name};

pub enum VariableResolution {
    Special,
    Lexical(Option<Value>),
}

impl Environment {
    pub(crate) fn declare_special(&self, name: impl AsRef<str>) {
        self.0
            .borrow_mut()
            .special_names
            .insert(intern_name(name.as_ref()));
    }

    pub(crate) fn declare_special_exact(&self, name: impl AsRef<str>) {
        self.0
            .borrow_mut()
            .exact_special_names
            .insert(name.as_ref().to_string());
    }

    pub(crate) fn has_local_special(&self, names: &[String]) -> bool {
        let frame = self.0.borrow();
        names
            .iter()
            .any(|name| frame.special_names.contains(&intern_name(name)))
    }

    pub(crate) fn has_local_special_exact(&self, name: &str) -> bool {
        self.0.borrow().exact_special_names.contains(name)
    }

    pub(crate) fn resolve(&self, names: &[String]) -> VariableResolution {
        let (special, value, parent) = {
            let frame = self.0.borrow();
            let special = names
                .iter()
                .any(|name| frame.special_names.contains(&intern_name(name)));
            let value = names
                .iter()
                .find_map(|name| frame.values.get(&intern_name(name)).cloned());
            (special, value, frame.parent.clone())
        };
        if special {
            VariableResolution::Special
        } else if value.is_some() {
            VariableResolution::Lexical(value)
        } else {
            parent.map_or(VariableResolution::Lexical(None), |environment| {
                environment.resolve(names)
            })
        }
    }

    pub(crate) fn resolve_exact(&self, name: &str) -> VariableResolution {
        let (special, value, parent) = {
            let frame = self.0.borrow();
            (
                frame.exact_special_names.contains(name),
                frame.exact_values.get(name).cloned(),
                frame.parent.clone(),
            )
        };
        if special {
            VariableResolution::Special
        } else if value.is_some() {
            VariableResolution::Lexical(value)
        } else {
            parent.map_or(VariableResolution::Lexical(None), |environment| {
                environment.resolve_exact(name)
            })
        }
    }

    /// Defines a case-insensitive variable binding.
    pub fn define(&self, name: impl AsRef<str>, value: Value) {
        let key = intern_name(name.as_ref());
        self.0.borrow_mut().values.insert(key, value);
    }

    pub(crate) fn define_exact(&self, name: impl AsRef<str>, value: Value) {
        self.0
            .borrow_mut()
            .exact_values
            .insert(intern_exact_name(name.as_ref()).to_string(), value);
    }

    /// Looks up a case-insensitive variable binding through the parent chain.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<Value> {
        self.lookup_interned(&intern_name(name))
    }

    pub(crate) fn lookup_interned(&self, key: &std::rc::Rc<str>) -> Option<Value> {
        let (value, parent) = {
            let frame = self.0.borrow();
            (frame.values.get(key).cloned(), frame.parent.clone())
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup_interned(key)))
    }

    pub(crate) fn lookup_exact(&self, name: &str) -> Option<Value> {
        self.lookup_exact_interned(&intern_exact_name(name))
    }

    pub(crate) fn lookup_exact_interned(&self, key: &std::rc::Rc<str>) -> Option<Value> {
        let (value, parent) = {
            let frame = self.0.borrow();
            (
                frame.exact_values.get(key.as_ref()).cloned(),
                frame.parent.clone(),
            )
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup_exact_interned(key)))
    }

    /// Updates the nearest existing case-insensitive variable binding.
    #[must_use]
    pub fn set(&self, name: &str, value: Value) -> bool {
        let key = intern_name(name);
        if self.0.borrow().values.contains_key(&key) {
            self.0.borrow_mut().values.insert(key, value);
            true
        } else {
            let parent = self.0.borrow().parent.clone();
            parent.is_some_and(|environment| environment.set(name, value))
        }
    }

    pub(crate) fn remove(&self, name: &str) -> bool {
        let key = intern_name(name);
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            (frame.values.remove(&key).is_some(), frame.parent.clone())
        };
        removed || parent.is_some_and(|environment| environment.remove(name))
    }

    pub(crate) fn set_exact(&self, name: &str, value: Value) -> bool {
        let key = intern_exact_name(name);
        if self.0.borrow().exact_values.contains_key(key.as_ref()) {
            self.0
                .borrow_mut()
                .exact_values
                .insert(key.to_string(), value);
            true
        } else {
            let parent = self.0.borrow().parent.clone();
            parent.is_some_and(|environment| environment.set_exact(name, value))
        }
    }

    pub(crate) fn remove_exact(&self, name: &str) -> bool {
        let key = intern_exact_name(name);
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            (
                frame.exact_values.remove(key.as_ref()).is_some(),
                frame.parent.clone(),
            )
        };
        removed || parent.is_some_and(|environment| environment.remove_exact(name))
    }
}

#[cfg(test)]
mod tests {
    use crate::Value;
    use crate::environment::Environment;

    fn assert_integer(value: Option<&Value>, expected: i64) {
        assert!(matches!(value, Some(Value::Integer(actual)) if *actual == expected));
    }

    #[test]
    fn lexical_bindings_update_and_remove_through_parent_chain() {
        let root = Environment::new();
        let child = root.child();
        root.define("Answer", Value::Integer(41));

        assert_integer(child.lookup("answer").as_ref(), 41);
        assert!(child.set("ANSWER", Value::Integer(42)));
        assert_integer(root.lookup("answer").as_ref(), 42);
        assert!(child.remove("answer"));
        assert!(root.lookup("answer").is_none());
        assert!(!child.set("missing", Value::Nil));
        assert!(!child.remove("missing"));
    }

    #[test]
    fn exact_bindings_preserve_case_and_update_parent() {
        let root = Environment::default();
        let child = root.child();
        root.define_exact("CaseSensitive", Value::Integer(7));

        assert_integer(child.lookup_exact("CaseSensitive").as_ref(), 7);
        assert!(child.lookup_exact("casesensitive").is_none());
        assert!(child.set_exact("CaseSensitive", Value::Integer(8)));
        assert_integer(root.lookup_exact("CaseSensitive").as_ref(), 8);
        assert!(child.remove_exact("CaseSensitive"));
        assert!(root.lookup_exact("CaseSensitive").is_none());
        assert!(!child.set_exact("missing", Value::Nil));
        assert!(!child.remove_exact("missing"));
    }
}
