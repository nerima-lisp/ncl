use crate::Value;
use crate::error::{ReturnValue, RuntimeError, SignaledError, ThrowTag};
use ncl_compiler::{CompileError, CompileErrorKind};
use ncl_syntax::{ReadError, ReadErrorKind, Span};

#[test]
fn condition_type_names_cover_read_and_compile_errors() {
    let read = RuntimeError::Read(Box::new(ReadError::new(
        ReadErrorKind::MissingDottedTail,
        Span::new(0, 1),
    )));
    assert_eq!(read.condition_type_name(), "READER-ERROR");

    let compile = RuntimeError::Compile(Box::new(CompileError::new(
        CompileErrorKind::Internal {
            message: "bad".to_owned(),
        },
        Span::new(0, 1),
    )));
    assert_eq!(compile.condition_type_name(), "COMPILER-ERROR");
}

#[test]
fn condition_type_names_cover_signaled_warning_and_error_variants() {
    let warning = RuntimeError::Signaled(Box::new(SignaledError {
        condition: "CUSTOM-CONDITION".to_owned(),
        condition_types: Box::default(),
        message: "warned".to_owned(),
        format_control: None,
        format_arguments: Box::default(),
        warning: true,
        span: None,
    }));
    assert_eq!(warning.condition_type_name(), "SIMPLE-WARNING");

    let error = RuntimeError::Signaled(Box::new(SignaledError {
        condition: "CUSTOM-ERROR".to_owned(),
        condition_types: Box::default(),
        message: "failed".to_owned(),
        format_control: None,
        format_arguments: Box::default(),
        warning: false,
        span: None,
    }));
    assert_eq!(error.condition_type_name(), "CUSTOM-ERROR");
}

#[test]
fn condition_type_names_cover_all_runtime_error_categories() {
    let cases = [
        (
            RuntimeError::UnboundVariable {
                name: "X".into(),
                span: None,
            },
            "UNBOUND-VARIABLE",
        ),
        (
            RuntimeError::NotCallable {
                value: "X".into(),
                span: None,
            },
            "TYPE-ERROR",
        ),
        (
            RuntimeError::Type {
                expected: "X".into(),
                actual: "Y".into(),
                span: None,
            },
            "TYPE-ERROR",
        ),
        (
            RuntimeError::Arity {
                function: "F".into(),
                expected: "1".into(),
                actual: 0,
            },
            "PROGRAM-ERROR",
        ),
        (
            RuntimeError::InvalidForm {
                message: "bad".into(),
                span: None,
            },
            "SIMPLE-ERROR",
        ),
        (
            RuntimeError::Package {
                message: "bad".into(),
                span: None,
            },
            "PACKAGE-ERROR",
        ),
        (
            RuntimeError::ReturnFrom {
                block: "B".into(),
                target: None,
                value: ReturnValue::new(Value::Nil),
                span: None,
            },
            "CONTROL-ERROR",
        ),
        (
            RuntimeError::Throw {
                tag: ThrowTag::new(Value::Nil),
                value: ReturnValue::new(Value::Nil),
                span: None,
            },
            "CONTROL-ERROR",
        ),
        (
            RuntimeError::InvokeRestart {
                name: "A".into(),
                value: ReturnValue::new(Value::Nil),
                arguments: Vec::new(),
                span: None,
            },
            "CONTROL-ERROR",
        ),
        (RuntimeError::NumericOverflow, "ARITHMETIC-ERROR"),
        (
            RuntimeError::Io {
                kind: std::io::ErrorKind::Other,
                message: "io".into(),
            },
            "FILE-ERROR",
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.condition_type_name(), expected);
    }
}
