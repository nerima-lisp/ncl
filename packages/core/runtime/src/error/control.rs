use std::fmt;

use crate::Value;

#[derive(Clone, Debug)]
/// A value returned through a non-local control transfer.
pub struct ReturnValue(Box<Value>);

impl ReturnValue {
    /// Wraps a runtime value.
    #[must_use]
    pub fn new(value: Value) -> Self {
        Self(Box::new(value))
    }

    /// Extracts the wrapped runtime value.
    #[must_use]
    pub fn into_value(self) -> Value {
        *self.0
    }
}

impl PartialEq for ReturnValue {
    fn eq(&self, other: &Self) -> bool {
        self.0.equal_value(&other.0)
    }
}

impl Eq for ReturnValue {}

#[derive(Clone, Debug)]
/// A tag used by `catch` and `throw` control transfers.
pub struct ThrowTag(Box<Value>);

impl ThrowTag {
    pub(crate) fn new(value: Value) -> Self {
        Self(Box::new(value))
    }

    pub(crate) fn matches(&self, value: &Value) -> bool {
        self.0.eq_value(value)
    }
}

impl PartialEq for ThrowTag {
    fn eq(&self, other: &Self) -> bool {
        self.matches(&other.0)
    }
}

impl Eq for ThrowTag {}

impl fmt::Display for ThrowTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value() -> Value {
        Value::Integer(7)
    }

    #[test]
    fn return_values_compare_by_lisp_equality_and_round_trip() {
        let returned = ReturnValue::new(value());
        assert_eq!(returned, ReturnValue::new(Value::Integer(7)));
        assert!(returned.into_value().equal_value(&value()));
    }

    #[test]
    fn throw_tags_use_identity_equality_and_display() {
        let tag = ThrowTag::new(Value::symbol("TAG"));
        assert!(tag.matches(&Value::symbol("TAG")));
        assert!(!tag.matches(&Value::symbol("OTHER")));
        assert_eq!(tag.to_string(), "TAG");
    }

    #[test]
    fn throw_tags_compare_equal_by_lisp_value_equality() {
        assert_eq!(
            ThrowTag::new(Value::symbol("TAG")),
            ThrowTag::new(Value::symbol("TAG"))
        );
        assert_ne!(
            ThrowTag::new(Value::symbol("TAG")),
            ThrowTag::new(Value::symbol("OTHER"))
        );
    }
}
