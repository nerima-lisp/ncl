use crate::Value;
use crate::environment::{Environment, normalize_name};

impl Environment {
    pub(crate) fn define_function(&self, name: impl AsRef<str>, value: Value) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().functions.insert(key, value);
    }

    pub(crate) fn define_function_exact(&self, name: impl AsRef<str>, value: Value) {
        self.0
            .borrow_mut()
            .exact_functions
            .insert(name.as_ref().to_string(), value);
    }

    pub(crate) fn lookup_function(&self, name: &str) -> Option<Value> {
        let key = normalize_name(name);
        let (value, parent) = {
            let frame = self.0.borrow();
            (frame.functions.get(&key).cloned(), frame.parent.clone())
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup_function(name)))
    }

    pub(crate) fn lookup_function_exact(&self, name: &str) -> Option<Value> {
        let (value, parent) = {
            let frame = self.0.borrow();
            (
                frame.exact_functions.get(name).cloned(),
                frame.parent.clone(),
            )
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup_function_exact(name)))
    }

    pub(crate) fn remove_function(&self, name: &str) -> bool {
        let key = normalize_name(name);
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            (frame.functions.remove(&key).is_some(), frame.parent.clone())
        };
        removed || parent.is_some_and(|environment| environment.remove_function(name))
    }

    pub(crate) fn remove_function_exact(&self, name: &str) -> bool {
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            (
                frame.exact_functions.remove(name).is_some(),
                frame.parent.clone(),
            )
        };
        removed || parent.is_some_and(|environment| environment.remove_function_exact(name))
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
    fn function_bindings_are_case_insensitive_and_exact_variants_are_distinct() {
        let root = Environment::new();
        let child = root.child();
        root.define_function("Print", Value::Integer(1));
        root.define_function_exact("Print", Value::Integer(2));

        assert_integer(child.lookup_function("print").as_ref(), 1);
        assert_integer(child.lookup_function_exact("Print").as_ref(), 2);
        assert!(child.lookup_function_exact("print").is_none());
    }

    #[test]
    fn function_bindings_remove_from_parent_and_report_missing_names() {
        let root = Environment::new();
        let child = root.child();
        root.define_function("Print", Value::Integer(1));
        root.define_function_exact("Print", Value::Integer(2));

        assert!(child.remove_function("print"));
        assert!(!child.remove_function("print"));
        assert!(child.remove_function_exact("Print"));
        assert!(!child.remove_function_exact("Print"));
    }
}
