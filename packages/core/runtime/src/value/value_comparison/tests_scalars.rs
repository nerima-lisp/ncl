#[cfg(test)]
mod tests {
    use crate::value::Value;

    #[test]
    fn eq_and_equal_cover_scalar_and_shared_container_semantics() {
        let rational = match Value::rational(3, 2) {
            Ok(value) => value,
            Err(error) => panic!("unexpected rational construction error: {error}"),
        };
        let scalar_cases = [
            (Value::Nil, Value::Boolean(false), true),
            (Value::Boolean(true), Value::Boolean(true), true),
            (Value::Integer(1), Value::Integer(1), true),
            (rational.clone(), rational, true),
            (Value::Float(1.5), Value::Float(1.5), true),
            (Value::Character('x'), Value::Character('x'), true),
            (Value::symbol("name"), Value::symbol("name"), true),
            (Value::keyword("name"), Value::keyword("name"), true),
            (
                Value::symbol_exact("name"),
                Value::symbol_exact("name"),
                true,
            ),
            (
                Value::keyword_exact("name"),
                Value::keyword_exact("name"),
                true,
            ),
            (Value::package("CL"), Value::package("CL"), true),
            (Value::Integer(1), Value::Integer(2), false),
            (Value::Integer(1), Value::Boolean(true), false),
        ];

        for (left, right, expected) in scalar_cases {
            assert_eq!(left.eq_value(&right), expected);
            assert_eq!(left.equal_value(&right), expected);
        }

        let list = Value::list(vec![Value::Integer(1)]);
        let same_list = list.clone();
        let equivalent_list = Value::list(vec![Value::Integer(1)]);
        assert!(list.eq_value(&same_list));
        assert!(!list.eq_value(&equivalent_list));
        assert!(list.equal_value(&equivalent_list));

        let vector = Value::vector(vec![Value::Integer(1)]);
        assert!(vector.equal_value(&Value::vector(vec![Value::Integer(1)])));
        let array = Value::array(vec![1], vec![Value::Integer(1)]);
        assert!(array.eq_value(&array));
        assert!(!array.eq_value(&Value::array(vec![1], vec![Value::Integer(1)])));
        assert!(array.equal_value(&Value::array(vec![1], vec![Value::Integer(1)])));
        let values = Value::values(vec![Value::Integer(1)]);
        assert!(values.eq_value(&values));
        assert!(values.equal_value(&Value::values(vec![Value::Integer(1)])));

        let hash_table = Value::hash_table("eq");
        assert!(hash_table.eq_value(&hash_table));
        assert!(!hash_table.eq_value(&Value::hash_table("eq")));

        let dotted = Value::dotted_list(vec![Value::Integer(1)], Value::Nil);
        assert!(dotted.equal_value(&Value::dotted_list(vec![Value::Integer(1)], Value::Nil)));
        assert!(!dotted.equal_value(&Value::dotted_list(
            vec![Value::Integer(1)],
            Value::Integer(2)
        )));
    }
}
