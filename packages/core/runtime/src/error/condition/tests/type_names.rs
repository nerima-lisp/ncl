use crate::Value;
use crate::error::{ReturnValue, RuntimeError, ThrowTag};

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
