use super::super::Value;

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
