use super::{build_x86_64_stub, record_boundary_error, unregister_heap_roots};
use ncl_object::{ObjectError, ThreadContext, Word, push_heap_root, unregister_heap_root};

#[test]
fn x86_stub_places_pinned_and_frame_arguments_in_sysv_stack_slots() -> Result<(), String> {
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
    if !bytes
        .windows(5)
        .any(|window| window == [0x48, 0x89, 0x6c, 0x24, 0x10])
    {
        return Err("caller frame pointer is not stored in the ninth argument slot".to_owned());
    }
    if !bytes
        .windows(5)
        .any(|window| window == [0x48, 0x8b, 0x44, 0x24, 0x28])
        || !bytes
            .windows(5)
            .any(|window| window == [0x48, 0x89, 0x44, 0x24, 0x18])
    {
        return Err("continuation PC is not stored in the tenth argument slot".to_owned());
    }
    Ok(())
}

#[test]
fn aarch64_stub_saves_frame_and_calls_dispatcher() -> Result<(), String> {
    let bytes = super::boundary_stub::build_aarch64_stub(0x1122_3344_5566_7788)
        .map_err(|error| format!("aarch64 stub assembly: {error}"))?;
    let contains = |instruction| bytes.windows(4).any(|window| window == instruction);
    // check-added-lines: allow(panic) test-only
    assert!(
        bytes.len() == 56
            && bytes.starts_with(&[0xff, 0x83, 0x00, 0xd1])
            && contains([0xfd, 0x03, 0x00, 0xf8])
            && contains([0xfe, 0x83, 0x00, 0xf8])
            && contains([0xfe, 0x03, 0x01, 0xf8])
            && contains([0xe6, 0x03, 0x10, 0xaa])
            && contains([0xe7, 0x03, 0x15, 0xaa])
            && contains([0x20, 0x01, 0x3f, 0xd6])
            && contains([0xfe, 0x03, 0x41, 0xf8])
            && contains([0xff, 0x83, 0x00, 0x91])
            && bytes.ends_with(&[0xc0, 0x03, 0x5f, 0xd6])
    );
    Ok(())
}

#[test]
fn x86_stub_rejects_unrepresentable_dispatch_address() {
    // check-added-lines: allow(panic) test-only
    assert!(build_x86_64_stub(u64::MAX).is_err());
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

#[test]
fn unregister_heap_roots_attempts_every_token_after_an_error() -> Result<(), String> {
    let object = ncl_object::Runtime::new().map_err(|error| format!("runtime init: {error}"))?;
    let mut stale_value = Word::NIL;
    let stale_token = push_heap_root(&object, &mut stale_value);
    unregister_heap_root(&object, stale_token)
        .map_err(|error| format!("initial unregister: {error}"))?;
    let mut live_value = Word::TRUE;
    let live_token = push_heap_root(&object, &mut live_value);

    let result = unregister_heap_roots(&object, [stale_token, live_token]);
    if result != Err(ObjectError::RootStackCorrupted) {
        return Err(format!("expected root error, got {result:?}"));
    }
    let second_unregister = unregister_heap_root(&object, live_token);
    if second_unregister != Err(ObjectError::RootStackCorrupted) {
        return Err(format!(
            "live token was not released: {second_unregister:?}"
        ));
    }
    Ok(())
}
