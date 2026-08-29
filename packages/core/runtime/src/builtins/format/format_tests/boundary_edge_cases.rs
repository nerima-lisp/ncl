use crate::builtins::format::boundaries;

#[test]
fn finds_directive_end_past_a_mismatched_nested_terminator() {
    let characters = "~{~)~}".chars().collect::<Vec<_>>();
    assert_eq!(boundaries::format_iteration_end(&characters, 1), Ok(4));
}

#[test]
fn rejects_directive_end_when_a_tilde_has_no_directive_character() {
    let characters = "~{~".chars().collect::<Vec<_>>();
    let Err(error) = boundaries::format_iteration_end(&characters, 1) else {
        panic!("truncated iteration directive should fail");
    };
    assert!(error.to_string().contains("format iteration is missing ~}"));
}

#[test]
fn propagates_prefix_parse_errors_from_boundary_finders() {
    let characters = "~{~-".chars().collect::<Vec<_>>();
    let Err(error) = boundaries::format_iteration_end(&characters, 1) else {
        panic!("malformed numeric prefix should fail");
    };
    assert!(
        error
            .to_string()
            .contains("format numeric parameter needs digits")
    );

    let body = "~-".chars().collect::<Vec<_>>();
    let Err(error) = boundaries::format_choice_clauses(&body) else {
        panic!("malformed numeric prefix should fail");
    };
    assert!(
        error
            .to_string()
            .contains("format numeric parameter needs digits")
    );
}

#[test]
fn rejects_format_choice_clause_ending_after_a_tilde() {
    let body = "abc~".chars().collect::<Vec<_>>();
    let Err(error) = boundaries::format_choice_clauses(&body) else {
        panic!("truncated choice clause should fail");
    };
    assert!(
        error
            .to_string()
            .contains("format choice clause ends after a tilde")
    );
}

#[test]
fn tracks_nested_justification_and_case_delimiters_inside_a_format_choice_clause() {
    let justification_body = "~<foo~>".chars().collect::<Vec<_>>();
    let clauses = boundaries::format_choice_clauses(&justification_body)
        .unwrap_or_else(|error| panic!("nested justification clause should parse: {error}"));
    assert_eq!(clauses.len(), 1);
    assert_eq!(
        clauses[0].0.iter().collect::<String>(),
        "~<foo~>".to_string()
    );

    let case_body = "~(foo~)".chars().collect::<Vec<_>>();
    let clauses = boundaries::format_choice_clauses(&case_body)
        .unwrap_or_else(|error| panic!("nested case-conversion clause should parse: {error}"));
    assert_eq!(clauses.len(), 1);
    assert_eq!(
        clauses[0].0.iter().collect::<String>(),
        "~(foo~)".to_string()
    );
}
