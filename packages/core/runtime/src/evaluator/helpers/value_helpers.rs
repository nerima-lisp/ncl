use super::Value;

pub(in crate::evaluator) fn macro_dotted_parts(value: &Value) -> Option<(Vec<Value>, Value)> {
    match value {
        Value::Nil => Some((Vec::new(), Value::Nil)),
        Value::List(values) => Some((values.as_ref().clone(), Value::Nil)),
        Value::DottedList { items, tail } => {
            let mut values = items.as_ref().clone();
            match tail.as_ref() {
                Value::Nil => Some((values, Value::Nil)),
                Value::List(more) => {
                    values.extend(more.as_ref().iter().cloned());
                    Some((values, Value::Nil))
                }
                Value::DottedList { .. } => {
                    let (more, dotted_tail) = macro_dotted_parts(tail)?;
                    values.extend(more);
                    Some((values, dotted_tail))
                }
                other => Some((values, other.clone())),
            }
        }
        _ => None,
    }
}

pub(in crate::evaluator) fn quasiquote_marker(name: &str, value: Value) -> Value {
    Value::list(vec![Value::symbol(name), value])
}

#[cfg(test)]
mod tests {
    use super::{macro_dotted_parts, quasiquote_marker};
    use crate::Value;

    fn render(result: Option<(Vec<Value>, Value)>) -> String {
        let (items, tail) = result.unwrap_or_else(|| panic!("expected a dotted-parts result"));
        format!(
            "({}) . {tail}",
            items
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        )
    }

    #[test]
    fn nil_has_no_items_and_a_nil_tail() {
        assert_eq!(render(macro_dotted_parts(&Value::Nil)), "() . NIL");
    }

    #[test]
    fn a_proper_dotted_list_keeps_its_items_and_nil_tail() {
        let value = Value::dotted_list(vec![Value::Integer(1), Value::Integer(2)], Value::Nil);
        assert_eq!(render(macro_dotted_parts(&value)), "(1 2) . NIL");
    }

    #[test]
    fn a_list_tail_is_merged_into_the_items() {
        let value = Value::dotted_list(
            vec![Value::Integer(1)],
            Value::list(vec![Value::Integer(2), Value::Integer(3)]),
        );
        assert_eq!(render(macro_dotted_parts(&value)), "(1 2 3) . NIL");
    }

    #[test]
    fn nested_dotted_tails_are_flattened_recursively() {
        let value = Value::dotted_list(
            vec![Value::Integer(1)],
            Value::dotted_list(vec![Value::Integer(2)], Value::Integer(3)),
        );
        assert_eq!(render(macro_dotted_parts(&value)), "(1 2) . 3");
    }

    #[test]
    fn a_scalar_tail_is_kept_as_the_final_cdr() {
        let value = Value::dotted_list(vec![Value::Integer(1)], Value::Integer(9));
        assert_eq!(render(macro_dotted_parts(&value)), "(1) . 9");
    }

    #[test]
    fn non_list_values_have_no_dotted_parts() {
        assert!(macro_dotted_parts(&Value::Integer(1)).is_none());
    }

    #[test]
    fn quasiquote_marker_wraps_the_value_with_its_tag_symbol() {
        assert_eq!(
            quasiquote_marker("QUOTE", Value::Integer(5)).to_string(),
            "(QUOTE 5)"
        );
    }
}
