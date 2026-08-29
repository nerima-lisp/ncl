use crate::builtins::format::justification_clauses;

#[test]
fn splits_plain_text_clauses_around_semicolons() {
    let body = "abc~;def".chars().collect::<Vec<_>>();
    let clauses = justification_clauses::format_justification_clauses(&body)
        .unwrap_or_else(|error| panic!("plain-text clauses should parse: {error}"));
    let rendered = clauses
        .iter()
        .map(|clause| clause.iter().collect::<String>())
        .collect::<Vec<_>>();
    assert_eq!(rendered, vec!["abc".to_string(), "def".to_string()]);
}

#[test]
fn round_trips_matching_nested_directives() {
    let body = "~(case~)".chars().collect::<Vec<_>>();
    let clauses = justification_clauses::format_justification_clauses(&body)
        .unwrap_or_else(|error| panic!("nested directive should parse: {error}"));
    assert_eq!(clauses.len(), 1);
    assert_eq!(
        clauses[0].iter().collect::<String>(),
        "~(case~)".to_string()
    );
}

#[test]
fn rejects_malformed_justification_clauses_from_table_cases() {
    let cases = [
        ("abc~", "format justification clause ends after a tilde"),
        ("~(foo~]", "mismatched format justification terminator ~]"),
        ("foo~]", "unexpected format justification terminator ~]"),
        (
            "foo~:;bar",
            "format justification does not support modifiers on ~;",
        ),
        (
            "foo~@;bar",
            "format justification does not support modifiers on ~;",
        ),
        (
            "~(foo",
            "format justification contains an unclosed nested directive",
        ),
    ];

    for (control, expected_message) in cases {
        let body = control.chars().collect::<Vec<_>>();
        let Err(error) = justification_clauses::format_justification_clauses(&body) else {
            panic!("malformed justification clause should fail: {control}");
        };
        assert!(
            error.to_string().contains(expected_message),
            "{control}: {error}"
        );
    }
}
