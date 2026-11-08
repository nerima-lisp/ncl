use ncl_object::{HeapRootedWord, Word};
use ncl_sys::{NativeError, NativeOperation};

use crate::{Runtime, RuntimeError};

#[test]
fn new_runtime_error_variants_keep_their_display() -> Result<(), String> {
    let runtime = Runtime::new().map_err(|error| format!("runtime: {error:?}"))?;
    let condition = HeapRootedWord::new(&runtime.context, Word::NIL)
        .map_err(|error| format!("root condition: {error:?}"))?;
    let pending_with_report = RuntimeError::PendingCondition {
        condition,
        report: Some("native condition report".to_owned()),
    };
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        pending_with_report.to_string(),
        "native error: native condition report"
    );
    drop(pending_with_report);

    let condition = HeapRootedWord::new(&runtime.context, Word::NIL)
        .map_err(|error| format!("root condition: {error:?}"))?;
    let pending_without_report = RuntimeError::PendingCondition {
        condition,
        report: None,
    };
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        pending_without_report.to_string(),
        "pending SIMPLE-ERROR condition"
    );
    drop(pending_without_report);

    let undefined = RuntimeError::UndefinedFunction {
        name: "LIST".to_owned(),
    };
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        undefined.to_string(),
        "undefined function UNDEFINED-FUNCTION: LIST"
    );

    let failure = crate::native_error::native_failure(NativeError::TypeMismatch {
        operation: NativeOperation::Car,
        operand: 0,
        value: Word::fixnum(7),
    });
    let RuntimeError::NativeFailure { error, condition } = failure else {
        return Err("native_failure did not create NativeFailure".to_owned());
    };
    let failure = RuntimeError::NativeFailure { error, condition };
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        failure.to_string(),
        format!("native failure {error:?}: {condition:?}")
    );
    Ok(())
}
