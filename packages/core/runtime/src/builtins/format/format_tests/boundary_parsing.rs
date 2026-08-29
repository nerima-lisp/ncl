use crate::RuntimeError;
use crate::builtins::format::boundaries;

#[test]
fn parses_format_choice_boundaries_from_table_cases() {
    let cases = [
        ("~A~:;~A", 2, vec![false, true]),
        ("~A~[~A~]", 1, vec![false]),
    ];

    for (control, expected_count, expected_defaults) in cases {
        let body = control.chars().collect::<Vec<_>>();
        let clauses = boundaries::format_choice_clauses(&body)
            .unwrap_or_else(|error| panic!("format choice should parse: {error}"));
        assert_eq!(clauses.len(), expected_count, "{control}");
        assert_eq!(
            clauses
                .iter()
                .map(|(_, default)| *default)
                .collect::<Vec<_>>(),
            expected_defaults,
            "{control}"
        );
    }
}

#[test]
fn rejects_malformed_format_boundaries_from_table_cases() {
    let cases = [
        (
            "~A~[~A",
            "format choice contains an unclosed nested directive",
        ),
        ("~A~]", "unexpected format choice terminator ~]"),
        (
            "~A~@;",
            "at-sign modifier is not supported on a format choice clause",
        ),
    ];

    for (control, expected_message) in cases {
        let body = control.chars().collect::<Vec<_>>();
        let Err(error) = boundaries::format_choice_clauses(&body) else {
            panic!("malformed format choice should fail: {control}");
        };
        assert!(
            error.to_string().contains(expected_message),
            "{control}: {error}"
        );
    }
}

#[test]
fn parses_nested_format_boundaries_from_table_cases() {
    type BoundaryFinder = fn(&[char], usize) -> Result<usize, RuntimeError>;
    let cases: [(&str, BoundaryFinder, usize); 4] = [
        ("~{item ~[choice~]~}", boundaries::format_iteration_end, 17),
        ("~[item ~<justified~>~]", boundaries::format_choice_end, 20),
        (
            "~<item ~(case~)~>",
            boundaries::format_justification_end,
            15,
        ),
        (
            "~(item ~{iteration~}~)",
            boundaries::format_case_conversion_end,
            20,
        ),
    ];

    for (control, find_end, expected) in cases {
        let characters = control.chars().collect::<Vec<_>>();
        assert_eq!(find_end(&characters, 1), Ok(expected), "{control}");
    }
}

#[test]
fn rejects_missing_format_boundaries_from_table_cases() {
    type BoundaryFinder = fn(&[char], usize) -> Result<usize, RuntimeError>;
    let cases: [(BoundaryFinder, &str); 4] = [
        (
            boundaries::format_iteration_end,
            "format iteration is missing ~}",
        ),
        (boundaries::format_choice_end, "format choice is missing ~]"),
        (
            boundaries::format_justification_end,
            "format justification is missing ~>",
        ),
        (
            boundaries::format_case_conversion_end,
            "format case conversion is missing ~)",
        ),
    ];

    for (find_end, expected_message) in cases {
        let characters = "~A".chars().collect::<Vec<_>>();
        let Err(error) = find_end(&characters, 1) else {
            panic!("missing format boundary should fail");
        };
        assert!(error.to_string().contains(expected_message));
    }
}
