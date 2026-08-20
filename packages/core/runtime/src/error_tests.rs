use std::error::Error as StdError;

use ncl_compiler::{CompileError, CompileErrorKind};
use ncl_syntax::{ReadError, ReadErrorKind, Span};

use super::*;

fn span() -> Span {
    Span::new(3, 5)
}

fn read_error() -> ReadError {
    ReadError::new(ReadErrorKind::InvalidDispatch, Span::new(1, 2))
}

fn compile_error() -> CompileError {
    CompileError::new(
        CompileErrorKind::Internal {
            message: "internal compiler error".to_owned(),
        },
        Span::new(6, 8),
    )
}

fn return_value() -> ReturnValue {
    ReturnValue::new(Value::Integer(42))
}

fn throw_tag() -> ThrowTag {
    ThrowTag::new(Value::symbol("TAG"))
}

fn signaled(
    condition: &str,
    condition_types: &[&str],
    warning: bool,
    span: Option<Span>,
) -> RuntimeError {
    RuntimeError::Signaled {
        condition: condition.to_owned(),
        condition_types: Box::new(
            condition_types
                .iter()
                .map(|type_name| (*type_name).to_owned())
                .collect(),
        ),
        message: "signaled message".to_owned(),
        format_control: Some("~A".to_owned()),
        format_arguments: Box::new(vec![return_value()]),
        warning,
        span,
    }
}

#[test]
fn displays_every_runtime_error_variant() {
    let cases = vec![
        (
            "read",
            RuntimeError::Read(read_error()),
            "invalid reader dispatch at byte 1..2",
        ),
        (
            "compile",
            RuntimeError::Compile(compile_error()),
            "internal compiler error at byte 6..8",
        ),
        (
            "unbound variable without span",
            RuntimeError::UnboundVariable {
                name: "MISSING".to_owned(),
                span: None,
            },
            "unbound variable MISSING",
        ),
        (
            "unbound variable with span",
            RuntimeError::UnboundVariable {
                name: "MISSING".to_owned(),
                span: Some(span()),
            },
            "unbound variable MISSING at byte 3..5",
        ),
        (
            "not callable",
            RuntimeError::NotCallable {
                value: "1".to_owned(),
                span: Some(span()),
            },
            "1 is not callable at byte 3..5",
        ),
        (
            "arity",
            RuntimeError::Arity {
                function: "CONS".to_owned(),
                expected: "2".to_owned(),
                actual: 1,
            },
            "CONS expected 2 arguments, received 1",
        ),
        (
            "type",
            RuntimeError::Type {
                expected: "INTEGER".to_owned(),
                actual: "STRING".to_owned(),
                span: Some(span()),
            },
            "expected INTEGER, received STRING at byte 3..5",
        ),
        (
            "invalid form",
            RuntimeError::InvalidForm {
                message: "malformed form".to_owned(),
                span: None,
            },
            "malformed form",
        ),
        (
            "signaled",
            signaled("CUSTOM-ERROR", &[], false, Some(span())),
            "signaled message at byte 3..5",
        ),
        (
            "package",
            RuntimeError::Package {
                message: "package is missing".to_owned(),
                span: Some(span()),
            },
            "package is missing at byte 3..5",
        ),
        (
            "return-from",
            RuntimeError::ReturnFrom {
                block: "BLOCK".to_owned(),
                target: None,
                value: return_value(),
                span: Some(span()),
            },
            "return-from BLOCK at byte 3..5",
        ),
        (
            "go",
            RuntimeError::Go {
                tag: "TAG".to_owned(),
                target: None,
                span: None,
            },
            "go TAG",
        ),
        (
            "throw",
            RuntimeError::Throw {
                tag: throw_tag(),
                value: return_value(),
                span: Some(span()),
            },
            "throw TAG at byte 3..5",
        ),
        (
            "invoke-restart",
            RuntimeError::InvokeRestart {
                name: "USE-VALUE".to_owned(),
                value: return_value(),
                arguments: vec![return_value()],
                span: None,
            },
            "invoke-restart USE-VALUE",
        ),
        (
            "division by zero",
            RuntimeError::DivisionByZero,
            "division by zero",
        ),
        (
            "numeric overflow",
            RuntimeError::NumericOverflow,
            "numeric overflow",
        ),
        (
            "io",
            RuntimeError::Io("could not read file".to_owned()),
            "could not read file",
        ),
    ];

    assert!(!cases.is_empty());
    for (label, error, expected) in cases {
        assert_eq!(error.to_string(), expected, "{label}");
    }
}

#[test]
fn reports_condition_type_names_for_all_error_categories() {
    let cases = vec![
        (RuntimeError::Read(read_error()), "READER-ERROR"),
        (RuntimeError::Compile(compile_error()), "COMPILER-ERROR"),
        (
            RuntimeError::UnboundVariable {
                name: "X".to_owned(),
                span: None,
            },
            "UNBOUND-VARIABLE",
        ),
        (
            RuntimeError::NotCallable {
                value: "X".to_owned(),
                span: None,
            },
            "TYPE-ERROR",
        ),
        (
            RuntimeError::Arity {
                function: "F".to_owned(),
                expected: "1".to_owned(),
                actual: 0,
            },
            "PROGRAM-ERROR",
        ),
        (
            RuntimeError::InvalidForm {
                message: "bad".to_owned(),
                span: None,
            },
            "SIMPLE-ERROR",
        ),
        (signaled("CUSTOM-ERROR", &[], false, None), "CUSTOM-ERROR"),
        (
            signaled("CUSTOM-WARNING", &[], true, None),
            "SIMPLE-WARNING",
        ),
        (
            RuntimeError::Package {
                message: "bad package".to_owned(),
                span: None,
            },
            "PACKAGE-ERROR",
        ),
        (
            RuntimeError::ReturnFrom {
                block: "B".to_owned(),
                target: None,
                value: return_value(),
                span: None,
            },
            "CONTROL-ERROR",
        ),
        (
            RuntimeError::Go {
                tag: "T".to_owned(),
                target: None,
                span: None,
            },
            "CONTROL-ERROR",
        ),
        (
            RuntimeError::Throw {
                tag: throw_tag(),
                value: return_value(),
                span: None,
            },
            "CONTROL-ERROR",
        ),
        (
            RuntimeError::InvokeRestart {
                name: "R".to_owned(),
                value: return_value(),
                arguments: Vec::new(),
                span: None,
            },
            "CONTROL-ERROR",
        ),
        (RuntimeError::DivisionByZero, "DIVISION-BY-ZERO"),
        (RuntimeError::NumericOverflow, "ARITHMETIC-ERROR"),
        (RuntimeError::Io("io".to_owned()), "FILE-ERROR"),
    ];

    assert!(!cases.is_empty());
    for (error, expected) in cases {
        assert_eq!(error.condition_type_name(), expected);
    }
}

#[test]
fn matches_normalized_conditions_and_control_flow_boundaries() {
    let custom_error = signaled("custom-error", &["base-condition"], false, None);
    assert!(custom_error.matches_condition(":custom-error"));
    assert!(custom_error.matches_condition("BASE-CONDITION"));
    assert!(custom_error.matches_condition("simple-condition"));
    assert!(custom_error.matches_condition("condition"));
    assert!(!custom_error.matches_condition("error"));
    assert!(!custom_error.matches_condition("serious-condition"));

    let built_in_error = signaled("type-error", &[], false, None);
    assert!(built_in_error.matches_condition("ERROR"));
    assert!(built_in_error.matches_condition("SERIOUS-CONDITION"));

    let warning = signaled("custom-warning", &["warning-base"], true, None);
    assert!(warning.matches_condition("CONDITION"));
    assert!(warning.matches_condition(":custom-warning"));
    assert!(warning.matches_condition("WARNING"));
    assert!(warning.matches_condition("WARNING-BASE"));
    assert!(!warning.matches_condition("ERROR"));
    assert!(!warning.matches_condition("SERIOUS-CONDITION"));
    assert!(!warning.matches_condition("SIMPLE-CONDITION"));

    for error in [
        RuntimeError::Read(read_error()),
        RuntimeError::Compile(compile_error()),
        RuntimeError::DivisionByZero,
        RuntimeError::Io("io".to_owned()),
    ] {
        assert!(error.matches_condition("condition"));
        assert!(error.matches_condition("ERROR"));
        assert!(error.matches_condition("serious-condition"));
    }

    assert!(RuntimeError::DivisionByZero.matches_condition("DIVISION-BY-ZERO"));
    assert!(RuntimeError::DivisionByZero.matches_condition("ARITHMETIC-ERROR"));
    assert!(!RuntimeError::DivisionByZero.matches_condition("TYPE-ERROR"));
    assert!(RuntimeError::NumericOverflow.matches_condition("ARITHMETIC-ERROR"));
    assert!(!RuntimeError::NumericOverflow.matches_condition("DIVISION-BY-ZERO"));

    let type_error = RuntimeError::Type {
        expected: "INTEGER".to_owned(),
        actual: "STRING".to_owned(),
        span: None,
    };
    assert!(type_error.matches_condition("TYPE-ERROR"));

    for error in [
        RuntimeError::ReturnFrom {
            block: "B".to_owned(),
            target: None,
            value: return_value(),
            span: None,
        },
        RuntimeError::Go {
            tag: "T".to_owned(),
            target: None,
            span: None,
        },
        RuntimeError::Throw {
            tag: throw_tag(),
            value: return_value(),
            span: None,
        },
        RuntimeError::InvokeRestart {
            name: "R".to_owned(),
            value: return_value(),
            arguments: Vec::new(),
            span: None,
        },
    ] {
        assert!(!error.matches_condition("CONDITION"));
        assert!(!error.matches_condition("CONTROL-ERROR"));
    }
}

#[test]
fn supports_value_wrappers_and_error_sources() {
    let first = ReturnValue::new(Value::list(vec![Value::Integer(1)]));
    let second = ReturnValue::new(Value::list(vec![Value::Integer(1)]));
    assert_eq!(first, second);
    assert_eq!(first.clone().into_value().to_string(), "(1)");

    let tag = throw_tag();
    assert!(tag.matches(&Value::symbol("TAG")));
    assert_eq!(tag, ThrowTag::new(Value::symbol("TAG")));
    assert_ne!(tag, ThrowTag::new(Value::symbol("OTHER")));

    let read = RuntimeError::Read(read_error());
    let compile = RuntimeError::Compile(compile_error());
    let io = RuntimeError::Io("io".to_owned());
    assert!(StdError::source(&read).is_some());
    assert!(StdError::source(&compile).is_some());
    assert!(StdError::source(&io).is_none());
}
