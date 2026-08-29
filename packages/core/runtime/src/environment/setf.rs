use crate::Value;
use crate::environment::{Environment, intern_name};

impl Environment {
    pub(crate) fn define_setf_function(&self, name: impl AsRef<str>, value: Value) {
        let key = intern_name(name.as_ref());
        self.0.borrow_mut().setf_functions.insert(key, value);
    }

    pub(crate) fn lookup_setf_function(&self, name: &str) -> Option<Value> {
        let key = intern_name(name);
        let (value, parent) = {
            let frame = self.0.borrow();
            (
                frame.setf_functions.get(&key).cloned(),
                frame.parent.clone(),
            )
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup_setf_function(name)))
    }

    pub(crate) fn define_setf_expander(&self, name: impl AsRef<str>, value: Value) {
        let key = intern_name(name.as_ref());
        self.0.borrow_mut().setf_expanders.insert(key, value);
    }

    pub(crate) fn lookup_setf_expander(&self, name: &str) -> Option<Value> {
        let key = intern_name(name);
        let (value, parent) = {
            let frame = self.0.borrow();
            (
                frame.setf_expanders.get(&key).cloned(),
                frame.parent.clone(),
            )
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup_setf_expander(name)))
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
    fn setf_function_and_expander_bindings_resolve_case_insensitively() {
        let root = Environment::new();
        let child = root.child();

        root.define_setf_function("place", Value::Integer(1));
        root.define_setf_expander("place", Value::Integer(2));
        assert_integer(child.lookup_setf_function("PLACE").as_ref(), 1);
        assert_integer(child.lookup_setf_expander("PLACE").as_ref(), 2);
    }
}
