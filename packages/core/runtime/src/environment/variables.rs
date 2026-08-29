use crate::Value;
use crate::environment::{Environment, normalize_name};

impl Environment {
    /// Defines a case-insensitive variable binding.
    pub fn define(&self, name: impl AsRef<str>, value: Value) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().values.insert(key, value);
    }

    pub(crate) fn define_exact(&self, name: impl AsRef<str>, value: Value) {
        self.0
            .borrow_mut()
            .exact_values
            .insert(name.as_ref().to_string(), value);
    }

    /// Looks up a case-insensitive variable binding through the parent chain.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<Value> {
        let key = normalize_name(name);
        let (value, parent) = {
            let frame = self.0.borrow();
            (frame.values.get(&key).cloned(), frame.parent.clone())
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup(name)))
    }

    pub(crate) fn lookup_exact(&self, name: &str) -> Option<Value> {
        let (value, parent) = {
            let frame = self.0.borrow();
            (frame.exact_values.get(name).cloned(), frame.parent.clone())
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup_exact(name)))
    }

    /// Updates the nearest existing case-insensitive variable binding.
    #[must_use]
    pub fn set(&self, name: &str, value: Value) -> bool {
        let key = normalize_name(name);
        if self.0.borrow().values.contains_key(&key) {
            self.0.borrow_mut().values.insert(key, value);
            true
        } else {
            let parent = self.0.borrow().parent.clone();
            parent.is_some_and(|environment| environment.set(name, value))
        }
    }

    pub(crate) fn remove(&self, name: &str) -> bool {
        let key = normalize_name(name);
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            (frame.values.remove(&key).is_some(), frame.parent.clone())
        };
        removed || parent.is_some_and(|environment| environment.remove(name))
    }

    pub(crate) fn set_exact(&self, name: &str, value: Value) -> bool {
        if self.0.borrow().exact_values.contains_key(name) {
            self.0
                .borrow_mut()
                .exact_values
                .insert(name.to_string(), value);
            true
        } else {
            let parent = self.0.borrow().parent.clone();
            parent.is_some_and(|environment| environment.set_exact(name, value))
        }
    }

    pub(crate) fn remove_exact(&self, name: &str) -> bool {
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            (
                frame.exact_values.remove(name).is_some(),
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
