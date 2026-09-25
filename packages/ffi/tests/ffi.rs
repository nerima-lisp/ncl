#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "tests assert on concrete values and errors"
)]

//! Foreign calls, dynamic loading, unmanaged memory, roots, and conditions.

use ncl_ffi::{
    AlienType, FfiError, SystemAreaPointer, alien_funcall, alien_routine, alien_size,
    allocate_system_memory, dlerror_message, load_shared_object, null_alien, sap_ref,
    signal_ffi_error, sys_requirements, with_rooted_objects,
};
use ncl_object::{Runtime, ThreadContext, Word, make_string};

/// Declare a runtime and a registered context as test locals, in that order so
/// the context drops before the runtime that owns its heap.
macro_rules! fixture {
    ($runtime:ident, $ctx:ident) => {
        let $runtime = Runtime::new().unwrap();
        let mut $ctx = ThreadContext::new();
        $ctx.register(&$runtime).unwrap();
    };
}

#[test]
fn alien_size_matches_the_type_table() {
    assert_eq!(alien_size(&AlienType::Int), 4);
    assert_eq!(alien_size(&AlienType::DoubleFloat), 8);
}

#[test]
fn null_alien_is_nil() {
    assert_eq!(null_alien(), Word::NIL);
}

#[test]
fn arity_mismatch_is_reported_before_marshalling() {
    fixture!(runtime, ctx);
    let routine = alien_routine("f", vec![AlienType::Int], AlienType::Int, true);
    let error = alien_funcall(&ctx, &runtime, &routine, &[]).unwrap_err();
    assert_eq!(
        error,
        FfiError::ArityMismatch {
            expected: 1,
            got: 0
        }
    );
}

#[test]
fn a_bad_argument_is_rejected_before_the_call() {
    fixture!(runtime, ctx);
    let routine = alien_routine("f", vec![AlienType::Int], AlienType::Int, true);
    let error = alien_funcall(&ctx, &runtime, &routine, &[Word::character(65)]).unwrap_err();
    assert!(matches!(error, FfiError::TypeMismatch { .. }));
}

#[test]
fn a_well_typed_call_reaches_the_missing_sys_primitive() {
    fixture!(runtime, ctx);
    let routine = alien_routine("f", vec![AlienType::Int], AlienType::Int, true);
    let error = alien_funcall(&ctx, &runtime, &routine, &[Word::fixnum(1)]).unwrap_err();
    assert_eq!(
        error,
        FfiError::MissingSysPrimitive(sys_requirements::CALL_FOREIGN_FUNCTION)
    );
}

#[test]
fn strlen_call_is_blocked_on_the_ncl_sys_call_primitive() {
    fixture!(runtime, ctx);
    let strlen = alien_routine("strlen", vec![AlienType::CString], AlienType::SizeT, false);
    let error = alien_funcall(&ctx, &runtime, &strlen, &[Word::fixnum(0x1000)]).unwrap_err();
    assert_eq!(
        error,
        FfiError::MissingSysPrimitive(sys_requirements::CALL_FOREIGN_FUNCTION)
    );
}

#[test]
fn dynamic_loading_reports_the_missing_wrappers() {
    let error = load_shared_object("libc.so.6", true).unwrap_err();
    assert_eq!(
        error,
        FfiError::MissingSysPrimitive(sys_requirements::DLOPEN_SHARED_OBJECT)
    );
    let error = dlerror_message().unwrap_err();
    assert_eq!(
        error,
        FfiError::MissingSysPrimitive(sys_requirements::DLERROR_MESSAGE)
    );
}

#[test]
fn unmanaged_memory_operations_report_the_missing_wrappers() {
    let error = allocate_system_memory(16).unwrap_err();
    assert_eq!(
        error,
        FfiError::MissingSysPrimitive(sys_requirements::ALLOCATE_SYSTEM_MEMORY)
    );
    fixture!(runtime, ctx);
    let error = sap_ref(
        &mut ctx,
        &runtime,
        &AlienType::Int,
        SystemAreaPointer::new(0x1000),
        0,
    )
    .unwrap_err();
    assert_eq!(
        error,
        FfiError::MissingSysPrimitive(sys_requirements::READ_SYSTEM_MEMORY)
    );
}

#[test]
fn rooted_objects_survive_a_collection() {
    fixture!(runtime, ctx);
    let string = make_string(&mut ctx, &runtime, &['x'; 64]).unwrap();
    let before = string;
    let after = with_rooted_objects(&mut ctx, &[string], |ctx, values| {
        ctx.collect(true)?;
        Ok(*values[0])
    })
    .unwrap();
    // The collector moved the string and rewrote the root slot in place, so the
    // value read after the collection differs from the stale pre-collection copy.
    assert_ne!(after, before);
    assert_eq!(ncl_object::string_length(&ctx, after).unwrap(), 64);
}

#[test]
fn ffi_error_signalling_needs_the_condition_hierarchy() {
    fixture!(runtime, ctx);
    let error = signal_ffi_error(&mut ctx, &runtime, "boom").unwrap_err();
    assert_eq!(error, FfiError::MissingConditionClass("SIMPLE-ERROR"));
}

#[test]
fn ffi_error_signals_a_simple_error_when_conditions_are_registered() {
    fixture!(runtime, ctx);
    ncl_conditions::register(&runtime).unwrap();
    let result = signal_ffi_error(&mut ctx, &runtime, "boom");
    assert!(result.is_ok() || matches!(result, Err(FfiError::Condition(_))));
}
