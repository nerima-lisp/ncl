#[cfg(test)]
mod tests {
    use crate::value::Value;

    #[test]
    fn comparisons_cover_identity_sensitive_and_fallback_pairs() {
        let string = Value::string("text");
        assert!(string.eq_value(&string));
        assert!(!string.eq_value(&Value::string("text")));
        assert!(string.equal_value(&Value::string("text")));

        let uninterned = Value::uninterned_symbol("name");
        assert!(uninterned.eq_value(&uninterned));
        assert!(!uninterned.eq_value(&Value::uninterned_symbol("name")));

        let values = [
            Value::Nil,
            Value::Boolean(true),
            Value::Integer(1),
            Value::Float(1.0),
            Value::Character('x'),
            Value::string("x"),
            Value::list(vec![Value::Integer(2)]),
        ];
        for (index, left) in values.iter().enumerate() {
            for (other_index, right) in values.iter().enumerate() {
                if index != other_index {
                    assert!(!left.eq_value(right));
                }
            }
        }
    }
}
