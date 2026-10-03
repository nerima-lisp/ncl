use super::{build_x86_64_stub, record_boundary_error};
use ncl_object::{ObjectError, ThreadContext};

#[test]
fn x86_stub_places_pinned_arguments_in_sysv_stack_slots() -> Result<(), String> {
    let bytes = build_x86_64_stub(0x1122_3344_5566_7788)
        .map_err(|error| format!("x86 stub assembly: {error}"))?;
    if !bytes
        .windows(4)
        .any(|window| window == [0x4c, 0x89, 0x14, 0x24])
    {
        return Err("function object is not stored in the seventh argument slot".to_owned());
    }
    if !bytes
        .windows(4)
        .any(|window| window == [0x4c, 0x89, 0x7c, 0x24])
    {
        return Err("thread context is not stored in the eighth argument slot".to_owned());
    }
    Ok(())
}

#[test]
fn boundary_non_local_exit_is_dropped_during_unwind() {
    let mut context = ThreadContext::new();
    context.set_non_local_exit(true);
    record_boundary_error(&mut context, ObjectError::NonLocalExit);
    // check-added-lines: allow(panic) test-only
    assert!(context.take_pending().is_none());
    // check-added-lines: allow(panic) test-only
    assert!(context.is_unwinding());
}

#[test]
fn boundary_non_local_exit_without_unwind_becomes_control_error() {
    let mut context = ThreadContext::new();
    record_boundary_error(&mut context, ObjectError::NonLocalExit);
    // check-added-lines: allow(panic) test-only
    assert_eq!(context.take_pending(), Some(ObjectError::ControlError));
}

#[test]
fn boundary_regular_error_is_recorded() {
    let mut context = ThreadContext::new();
    record_boundary_error(&mut context, ObjectError::TypeError);
    // check-added-lines: allow(panic) test-only
    assert_eq!(context.take_pending(), Some(ObjectError::TypeError));
}
