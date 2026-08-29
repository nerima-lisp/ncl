use crate::error::{RuntimeError, SignaledError};
use ncl_syntax::Span;

#[test]
fn condition_matching_normalizes_names_and_handles_control_errors() {
    let warning = RuntimeError::Signaled(Box::new(SignaledError {
        condition: "CUSTOM-WARNING".to_owned(),
        condition_types: vec!["SIMPLE-WARNING".to_owned()].into_boxed_slice(),
        message: "warning".to_owned(),
        format_control: None,
        format_arguments: Box::default(),
        warning: true,
        span: None,
    }));
    assert!(warning.matches_condition(":warning"));
    assert!(warning.matches_condition("condition"));
    assert!(!warning.matches_condition("error"));

    let error = RuntimeError::DivisionByZero;
    assert_eq!(error.condition_type_name(), "DIVISION-BY-ZERO");
    assert!(error.matches_condition("arithmetic-error"));
    assert_eq!(error.to_string(), "division by zero");

    let control = RuntimeError::Go {
        tag: "DONE".to_owned(),
        target: None,
        span: Some(Span::new(2, 5)),
    };
    assert!(!control.matches_condition("control-error"));
    assert_eq!(control.to_string(), "go DONE at byte 2..5");
}

#[test]
fn condition_matching_covers_builtin_condition_hierarchy() {
    let cases = [
        (RuntimeError::NumericOverflow, "arithmetic-error", true),
        (RuntimeError::NumericOverflow, "division-by-zero", false),
        (
            RuntimeError::Type {
                expected: "integer".to_owned(),
                actual: "string".to_owned(),
                span: None,
            },
            "condition",
            true,
        ),
        (
            RuntimeError::Type {
                expected: "integer".to_owned(),
                actual: "string".to_owned(),
                span: None,
            },
            "simple-condition",
            false,
        ),
        (
            RuntimeError::Arity {
                function: "f".to_owned(),
                expected: "one".to_owned(),
                actual: 2,
            },
            "program-error",
            true,
        ),
        (
            RuntimeError::Io {
                kind: std::io::ErrorKind::Other,
                message: "failed".to_owned(),
            },
            "file-error",
            true,
        ),
    ];

    for (error, condition, expected) in cases {
        assert_eq!(error.matches_condition(condition), expected, "{condition}");
    }
}
