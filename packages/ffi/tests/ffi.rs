#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "tests assert on concrete values and errors"
)]

//! Foreign calls, dynamic loading, unmanaged memory, roots, and conditions.

use ncl_ffi::{
    AlienType, FfiError, alien_funcall, alien_routine, alien_size, allocate_system_memory,
    dlerror_message, load_shared_object, null_alien, sap_ref, signal_ffi_error,
    with_rooted_objects,
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
    let error = alien_funcall(&mut ctx, &runtime, &routine, &[]).unwrap_err();
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
    let error = alien_funcall(&mut ctx, &runtime, &routine, &[Word::character(65)]).unwrap_err();
    assert!(matches!(error, FfiError::TypeMismatch { .. }));
}

#[test]
fn an_unlinked_well_typed_call_is_rejected() {
    fixture!(runtime, ctx);
    let routine = alien_routine("f", vec![AlienType::Int], AlienType::Int, true);
    let error = alien_funcall(&mut ctx, &runtime, &routine, &[Word::fixnum(1)]).unwrap_err();
    assert_eq!(error, FfiError::NullPointer);
}

#[test]
fn scalar_foreign_call_uses_the_dynamic_loader_address() {
    fixture!(runtime, ctx);
    let path = if cfg!(target_os = "macos") {
        "/usr/lib/libSystem.B.dylib"
    } else {
        "libc.so.6"
    };
    let object = load_shared_object(path, true).unwrap();
    let address = ncl_ffi::find_dynamic_foreign_symbol_address(&object, "abs")
        .unwrap()
        .address();
    let abs =
        alien_routine("abs", vec![AlienType::Int], AlienType::Int, true).with_address(address);
    let result = alien_funcall(&mut ctx, &runtime, &abs, &[Word::fixnum(-7)]).unwrap();
    assert_eq!(result.as_fixnum(), Some(7));
    ncl_ffi::unload_shared_object(object).unwrap();
}

#[test]
fn dynamic_loading_resolves_and_closes_a_platform_library() {
    let path = if cfg!(target_os = "macos") {
        "/usr/lib/libSystem.B.dylib"
    } else {
        "libc.so.6"
    };
    let object = load_shared_object(path, true).unwrap();
    let symbol = ncl_ffi::find_dynamic_foreign_symbol_address(&object, "abs").unwrap();
    assert_ne!(symbol.address(), 0);
    assert!(dlerror_message().unwrap().is_none());
    ncl_ffi::unload_shared_object(object).unwrap();
}

#[test]
fn unmanaged_memory_round_trips_through_sap() {
    let address = allocate_system_memory(16).unwrap();
    fixture!(runtime, ctx);
    ncl_ffi::sap_set(
        &ctx,
        &runtime,
        &AlienType::Int,
        address,
        0,
        Word::fixnum(42),
    )
    .unwrap();
    let value = sap_ref(&mut ctx, &runtime, &AlienType::Int, address, 0).unwrap();
    assert_eq!(value.as_fixnum(), Some(42));
    ncl_ffi::deallocate_system_memory(address, 16).unwrap();
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
    // Moving collection rewrites the root on aarch64. x86_64 uses conservative
    // stack scanning, so a live address may be pinned and remain unchanged.
    #[cfg(target_arch = "aarch64")]
    assert_ne!(after, before);
    #[cfg(target_arch = "x86_64")]
    assert_eq!(after, before);
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
