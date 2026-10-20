use ncl_object::ObjectError;
use ncl_sys::StorageCondition;

use crate::{Runtime, RuntimeError};

#[test]
fn make_closure_capacity_failure_propagates_before_capture_stores() -> Result<(), RuntimeError> {
    let mut runtime = Runtime::new()?;
    let _failure = crate::support::inject_make_closure_capacity_failure(&mut runtime.context);

    let result = runtime.eval("(let ((captured 7)) (lambda () captured))");

    assert!(matches!(
        result,
        Err(RuntimeError::Object(ObjectError::Storage(
            StorageCondition::CapacityExceeded
        )))
    ));
    assert!(!runtime.context.thread_access().runtime_error_pending());
    Ok(())
}
