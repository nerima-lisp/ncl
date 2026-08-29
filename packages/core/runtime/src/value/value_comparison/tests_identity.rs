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

    #[test]
    fn eq_value_uses_rc_identity_for_streams_vectors_and_dotted_lists() {
        let stream = Value::string_output_stream();
        assert!(stream.eq_value(&stream));
        assert!(!stream.eq_value(&Value::string_output_stream()));

        let vector = Value::vector(vec![Value::Integer(1)]);
        assert!(vector.eq_value(&vector));
        assert!(!vector.eq_value(&Value::vector(vec![Value::Integer(1)])));

        let dotted = Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2));
        assert!(dotted.eq_value(&dotted));
        assert!(!dotted.eq_value(&Value::dotted_list(
            vec![Value::Integer(1)],
            Value::Integer(2)
        )));
    }
}
