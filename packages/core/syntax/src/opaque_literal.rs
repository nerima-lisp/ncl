use std::{any::Any, fmt, rc::Rc};

/// A shared, type-erased literal payload. Clones share the wrapper allocation;
/// equality compares wrapper identity, not payload values.
#[derive(Clone)]
pub struct OpaqueLiteral(Rc<dyn Any>);

impl OpaqueLiteral {
    /// Retains a value without exposing its representation to the compiler.
    #[must_use]
    pub fn new<T: Any>(value: T) -> Self {
        Self(Rc::new(value))
    }

    /// Checks the Rust payload type, not the identity of its creating runtime.
    #[must_use]
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.0.downcast_ref()
    }
}

impl PartialEq for OpaqueLiteral {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for OpaqueLiteral {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("#<OPAQUE-LITERAL>")
    }
}

impl fmt::Display for OpaqueLiteral {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::OpaqueLiteral;
    use std::rc::Rc;

    #[test]
    fn cloned_literal_retains_ownership_and_allocation_identity() {
        let payload = Rc::new(String::from("retained"));
        let weak = Rc::downgrade(&payload);
        let literal = OpaqueLiteral::new(payload);
        let cloned = literal.clone();
        assert_eq!(literal, cloned);
        assert_ne!(
            literal,
            OpaqueLiteral::new(Rc::new(String::from("retained")))
        );

        drop(literal);
        assert!(weak.upgrade().is_some());
        assert_eq!(
            cloned
                .downcast_ref::<Rc<String>>()
                .map(|value| value.as_str()),
            Some("retained")
        );
        drop(cloned);
        assert!(weak.upgrade().is_none());
    }
}
