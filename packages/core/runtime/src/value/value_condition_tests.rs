#[cfg(test)]
mod tests {
    use super::super::Value;

    #[test]
    fn condition_query_methods_have_stable_boundaries() {
        let condition = Value::condition_from_parts_with_types(
            "SIMPLE-ERROR".to_owned(),
            vec!["SIMPLE-ERROR".to_owned()],
            vec![("DETAIL".to_owned(), Value::Integer(1))],
            "failed".to_owned(),
            Some("~A".to_owned()),
            vec![Value::Integer(1)],
        );
        assert!(condition.condition_is_type(":error"));
        assert!(
            condition
                .condition_slot("condition", "detail")
                .is_some_and(|value| value.equal_value(&Value::Integer(1)))
        );
        assert!(condition.set_condition_slot("error", "detail", Value::Integer(2)));
        assert!(
            condition
                .condition_slot("error", "detail")
                .is_some_and(|value| value.equal_value(&Value::Integer(2)))
        );
        assert!(condition.condition_slot("unrelated", "detail").is_none());
        assert!(!condition.set_condition_slot("error", "missing", Value::Nil));
        assert!(!Value::Nil.set_condition_slot("error", "detail", Value::Nil));
        assert_eq!(condition.condition_message(), Some("failed"));
        assert_eq!(condition.simple_condition_format_control(), Some("~A"));
        assert!(!Value::Nil.condition_is_type("error"));
    }

    #[test]
    fn condition_equality_checks_all_data_fields() {
        let condition = |actual_type: &str,
                         type_names: Vec<&str>,
                         slots: Vec<(&str, Value)>,
                         message: &str,
                         format_control: Option<&str>,
                         arguments: Vec<Value>| {
            Value::condition_from_parts_with_types(
                actual_type.to_owned(),
                type_names.into_iter().map(str::to_owned).collect(),
                slots
                    .into_iter()
                    .map(|(name, value)| (name.to_owned(), value))
                    .collect(),
                message.to_owned(),
                format_control.map(str::to_owned),
                arguments,
            )
        };
        let base = condition(
            "SIMPLE-ERROR",
            vec!["SIMPLE-ERROR"],
            vec![("DETAIL", Value::Integer(2))],
            "failed",
            Some("~A"),
            vec![Value::Integer(1)],
        );
        assert!(base.equal_value(&condition(
            "SIMPLE-ERROR",
            vec!["SIMPLE-ERROR"],
            vec![("DETAIL", Value::Integer(2))],
            "failed",
            Some("~A"),
            vec![Value::Integer(1)],
        )));

        let differences = [
            condition(
                "SIMPLE-WARNING",
                vec!["SIMPLE-ERROR"],
                vec![("DETAIL", Value::Integer(2))],
                "failed",
                Some("~A"),
                vec![Value::Integer(1)],
            ),
            condition(
                "SIMPLE-ERROR",
                vec!["ERROR"],
                vec![("DETAIL", Value::Integer(2))],
                "failed",
                Some("~A"),
                vec![Value::Integer(1)],
            ),
            condition(
                "SIMPLE-ERROR",
                vec!["SIMPLE-ERROR"],
                vec![("DETAIL", Value::Integer(2))],
                "changed",
                Some("~A"),
                vec![Value::Integer(1)],
            ),
            condition(
                "SIMPLE-ERROR",
                vec!["SIMPLE-ERROR"],
                vec![("DETAIL", Value::Integer(2))],
                "failed",
                None,
                vec![Value::Integer(1)],
            ),
            condition(
                "SIMPLE-ERROR",
                vec!["SIMPLE-ERROR"],
                vec![("DETAIL", Value::Integer(2))],
                "failed",
                Some("~A"),
                vec![Value::Integer(2)],
            ),
            condition(
                "SIMPLE-ERROR",
                vec!["SIMPLE-ERROR"],
                vec![("OTHER", Value::Integer(2))],
                "failed",
                Some("~A"),
                vec![Value::Integer(1)],
            ),
        ];
        assert!(
            differences
                .iter()
                .all(|different| !base.equal_value(different))
        );
    }

    #[test]
    fn condition_query_methods_cover_hierarchy_and_non_condition_targets() {
        let condition = |actual_type: &str| {
            Value::condition_from_parts_with_types(
                actual_type.to_owned(),
                vec![actual_type.to_owned()],
                Vec::new(),
                "msg".to_owned(),
                None,
                Vec::new(),
            )
        };
        for (actual_type, expected, matches) in [
            ("SIMPLE-WARNING", "WARNING", true),
            ("SIMPLE-CONDITION", "SIMPLE-ERROR", false),
            ("DIVISION-BY-ZERO", "ARITHMETIC-ERROR", true),
            ("ARITHMETIC-ERROR", "SERIOUS-CONDITION", true),
            ("TYPE-ERROR", "ERROR", true),
            ("UNBOUND-VARIABLE", "SERIOUS-CONDITION", true),
            ("CONTROL-ERROR", "ERROR", false),
            ("UNKNOWN-TYPE", "ERROR", false),
        ] {
            assert_eq!(condition(actual_type).condition_is_type(expected), matches);
        }

        let aliased = Value::condition_from_parts_with_types(
            "SIMPLE-ERROR".to_owned(),
            vec!["SIMPLE-ERROR".to_owned(), "CUSTOM-ALIAS".to_owned()],
            Vec::new(),
            "msg".to_owned(),
            None,
            Vec::new(),
        );
        assert!(aliased.condition_is_type("custom-alias"));

        assert_eq!(Value::Nil.condition_type_names(), None);
        assert!(Value::Nil.condition_slot("error", "detail").is_none());
        assert_eq!(Value::Nil.condition_message(), None);
        assert!(!condition("SIMPLE-ERROR").set_condition_slot(
            "warning",
            "detail",
            Value::Integer(2)
        ));
    }
}
