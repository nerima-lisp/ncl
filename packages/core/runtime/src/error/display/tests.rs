use crate::Value;
use crate::error::{ReturnValue, RuntimeError, SignaledError, ThrowTag};
use ncl_syntax::Span;

#[test]
fn runtime_error_display_includes_spans_and_variant_messages() {
    let span = Some(Span::new(1, 4));
    let errors = [
        (
            RuntimeError::UnboundVariable {
                name: "X".to_owned(),
                span,
            },
            "unbound variable X at byte 1..4",
        ),
        (
            RuntimeError::NotCallable {
                value: "7".to_owned(),
                span,
            },
            "7 is not callable at byte 1..4",
        ),
        (
            RuntimeError::Arity {
                function: "F".to_owned(),
                expected: "1".to_owned(),
                actual: 2,
            },
            "F expected 1 arguments, received 2",
        ),
        (
            RuntimeError::Type {
                expected: "INTEGER".to_owned(),
                actual: "STRING".to_owned(),
                span,
            },
            "expected INTEGER, received STRING at byte 1..4",
        ),
        (
            RuntimeError::InvalidForm {
                message: "bad form".to_owned(),
                span,
            },
            "bad form at byte 1..4",
        ),
        (
            RuntimeError::Package {
                message: "bad package".to_owned(),
                span,
            },
            "bad package at byte 1..4",
        ),
        (RuntimeError::NumericOverflow, "numeric overflow"),
        (RuntimeError::Io("io failed".to_owned()), "io failed"),
    ];
    for (error, expected) in errors {
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn runtime_error_display_covers_control_and_signaled_variants() {
    let span = Some(Span::new(3, 6));
    let signaled = RuntimeError::Signaled(Box::new(SignaledError {
        condition: "SIMPLE-ERROR".to_owned(),
        condition_types: Box::default(),
        message: "failed".to_owned(),
        format_control: None,
        format_arguments: Box::default(),
        warning: false,
        span,
    }));
    let cases = [
        (signaled, "failed at byte 3..6"),
        (
            RuntimeError::ReturnFrom {
                block: "BLOCK".to_owned(),
                target: None,
                value: ReturnValue::new(Value::Nil),
                span,
            },
            "return-from BLOCK at byte 3..6",
        ),
        (
            RuntimeError::Throw {
                tag: ThrowTag::new(Value::symbol("TAG")),
                value: ReturnValue::new(Value::Nil),
                span,
            },
            "throw TAG at byte 3..6",
        ),
        (
            RuntimeError::InvokeRestart {
                name: "ABORT".to_owned(),
                value: ReturnValue::new(Value::Nil),
                arguments: Vec::new(),
                span,
            },
            "invoke-restart ABORT at byte 3..6",
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
    }
}
