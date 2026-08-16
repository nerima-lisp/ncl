use super::{Environment, normalize_name};

impl Environment {
    pub(crate) fn define_function_documentation(
        &self,
        name: impl AsRef<str>,
        documentation: impl Into<String>,
    ) {
        let key = normalize_name(name.as_ref());
        self.0
            .borrow_mut()
            .function_documentation
            .insert(key, documentation.into());
    }

    pub(crate) fn define_function_documentation_exact(
        &self,
        name: impl AsRef<str>,
        documentation: impl Into<String>,
    ) {
        self.0
            .borrow_mut()
            .exact_function_documentation
            .insert(name.as_ref().to_string(), documentation.into());
    }

    pub(crate) fn set_function_documentation(
        &self,
        name: impl AsRef<str>,
        documentation: Option<String>,
    ) {
        let key = normalize_name(name.as_ref());
        let mut frame = self.0.borrow_mut();
        if let Some(documentation) = documentation {
            frame.function_documentation.insert(key, documentation);
        } else {
            frame.function_documentation.remove(&key);
        }
    }

    pub(crate) fn set_function_documentation_exact(
        &self,
        name: impl AsRef<str>,
        documentation: Option<String>,
    ) {
        let mut frame = self.0.borrow_mut();
        if let Some(documentation) = documentation {
            frame
                .exact_function_documentation
                .insert(name.as_ref().to_string(), documentation);
        } else {
            frame.exact_function_documentation.remove(name.as_ref());
        }
    }

    pub(crate) fn lookup_function_documentation(&self, name: &str) -> Option<String> {
        let key = normalize_name(name);
        let (documentation, parent) = {
            let frame = self.0.borrow();
            (
                frame.function_documentation.get(&key).cloned(),
                frame.parent.clone(),
            )
        };
        documentation.or_else(|| {
            parent.and_then(|environment| environment.lookup_function_documentation(name))
        })
    }

    pub(crate) fn lookup_function_documentation_exact(&self, name: &str) -> Option<String> {
        let (documentation, parent) = {
            let frame = self.0.borrow();
            (
                frame.exact_function_documentation.get(name).cloned(),
                frame.parent.clone(),
            )
        };
        documentation.or_else(|| {
            parent.and_then(|environment| environment.lookup_function_documentation_exact(name))
        })
    }

    pub(crate) fn define_variable_documentation(
        &self,
        name: impl AsRef<str>,
        documentation: impl Into<String>,
    ) {
        let key = normalize_name(name.as_ref());
        self.0
            .borrow_mut()
            .variable_documentation
            .insert(key, documentation.into());
    }

    pub(crate) fn define_variable_documentation_exact(
        &self,
        name: impl AsRef<str>,
        documentation: impl Into<String>,
    ) {
        self.0
            .borrow_mut()
            .exact_variable_documentation
            .insert(name.as_ref().to_string(), documentation.into());
    }

    pub(crate) fn set_variable_documentation(
        &self,
        name: impl AsRef<str>,
        documentation: Option<String>,
    ) {
        let key = normalize_name(name.as_ref());
        let mut frame = self.0.borrow_mut();
        if let Some(documentation) = documentation {
            frame.variable_documentation.insert(key, documentation);
        } else {
            frame.variable_documentation.remove(&key);
        }
    }

    pub(crate) fn set_variable_documentation_exact(
        &self,
        name: impl AsRef<str>,
        documentation: Option<String>,
    ) {
        let mut frame = self.0.borrow_mut();
        if let Some(documentation) = documentation {
            frame
                .exact_variable_documentation
                .insert(name.as_ref().to_string(), documentation);
        } else {
            frame.exact_variable_documentation.remove(name.as_ref());
        }
    }

    pub(crate) fn lookup_variable_documentation(&self, name: &str) -> Option<String> {
        let key = normalize_name(name);
        let (documentation, parent) = {
            let frame = self.0.borrow();
            (
                frame.variable_documentation.get(&key).cloned(),
                frame.parent.clone(),
            )
        };
        documentation.or_else(|| {
            parent.and_then(|environment| environment.lookup_variable_documentation(name))
        })
    }

    pub(crate) fn lookup_variable_documentation_exact(&self, name: &str) -> Option<String> {
        let (documentation, parent) = {
            let frame = self.0.borrow();
            (
                frame.exact_variable_documentation.get(name).cloned(),
                frame.parent.clone(),
            )
        };
        documentation.or_else(|| {
            parent.and_then(|environment| environment.lookup_variable_documentation_exact(name))
        })
    }
}
