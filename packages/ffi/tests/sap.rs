#![allow(clippy::unwrap_used, reason = "tests assert on concrete values")]

//! System area pointer arithmetic and comparisons.

use ncl_ffi::{
    AlienType, FfiError, SystemAreaPointer, sap_eq, sap_ge, sap_gt, sap_le, sap_lt, sap_ref,
    sap_set,
};
use ncl_object::{
    FunctionObject, ObjectError, Runtime, ThreadContext, Word, make_bignum_from_i128,
    with_rooted_slice,
};

fn fixture() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    (runtime, ctx)
}

#[test]
fn null_pointer_is_recognized() {
    assert!(SystemAreaPointer::null().is_null());
    assert!(!SystemAreaPointer::new(1).is_null());
}

#[test]
fn offsets_add_and_subtract_bytes() {
    let base = SystemAreaPointer::new(0x1000);
    assert_eq!(base.sap_plus(16).address(), 0x1010);
    assert_eq!(base.sap_minus(16).address(), 0x0ff0);
    assert_eq!(base.sap_plus(16).sap_minus(16), base);
}

#[test]
fn difference_is_signed() {
    let high = SystemAreaPointer::new(0x1020);
    let low = SystemAreaPointer::new(0x1000);
    assert_eq!(high.sap_difference(low), 32);
    assert_eq!(low.sap_difference(high), -32);
}

#[test]
fn sap_int_and_from_word_round_trip() {
    let (runtime, mut ctx) = fixture();
    for address in [0_u64, 1, 1 << 62, 1 << 63, u64::MAX] {
        let sap = SystemAreaPointer::new(usize::try_from(address).unwrap());
        let word = sap.to_word(&mut ctx, &runtime).unwrap();
        assert_eq!(SystemAreaPointer::from_word(&ctx, word).unwrap(), sap);
    }
}

#[test]
fn int_sap_rejects_negative_and_non_integer() {
    let (runtime, mut ctx) = fixture();
    assert!(matches!(
        SystemAreaPointer::from_word(&ctx, Word::fixnum(-1)),
        Err(FfiError::ValueOutOfRange { .. })
    ));
    assert!(matches!(
        SystemAreaPointer::from_word(&ctx, Word::NIL),
        Err(FfiError::TypeMismatch { .. })
    ));
    let too_wide = make_bignum_from_i128(&mut ctx, &runtime, i128::from(u64::MAX) + 1)
        .unwrap()
        .as_word();
    assert!(matches!(
        SystemAreaPointer::from_word(&ctx, too_wide),
        Err(FfiError::ValueOutOfRange { .. })
    ));
}

#[test]
fn registered_sap_builtins_convert_and_access_tracked_memory() {
    let (runtime, mut ctx) = fixture();
    ncl_ffi::register(&runtime).unwrap();
    let address = ncl_ffi::allocate_system_memory(2).unwrap();
    let address_word = address.to_word(&mut ctx, &runtime).unwrap();

    let plus = call_builtin(&mut ctx, &runtime, "SAP+", &[address_word, Word::fixnum(1)]).unwrap();
    assert_eq!(
        SystemAreaPointer::from_word(&ctx, plus).unwrap(),
        address.sap_plus(1)
    );
    assert_eq!(
        call_builtin(&mut ctx, &runtime, "SAP-INT", &[address_word]).unwrap(),
        address_word
    );
    assert_eq!(
        call_builtin(&mut ctx, &runtime, "INT-SAP", &[address_word]).unwrap(),
        address_word
    );

    call_builtin(
        &mut ctx,
        &runtime,
        "%SAP-SET-U8",
        &[address_word, Word::fixnum(0), Word::fixnum(255)],
    )
    .unwrap();
    assert_eq!(
        call_builtin(
            &mut ctx,
            &runtime,
            "SAP-REF-8",
            &[address_word, Word::fixnum(0)],
        )
        .unwrap(),
        Word::fixnum(255)
    );
    call_builtin(
        &mut ctx,
        &runtime,
        "%SAP-SET-I8",
        &[address_word, Word::fixnum(1), Word::fixnum(-128)],
    )
    .unwrap();
    assert_eq!(
        call_builtin(
            &mut ctx,
            &runtime,
            "SIGNED-SAP-REF-8",
            &[address_word, Word::fixnum(1)],
        )
        .unwrap(),
        Word::fixnum(-128)
    );
    assert_eq!(
        call_builtin(
            &mut ctx,
            &runtime,
            "%SAP-SET-I8",
            &[address_word, Word::fixnum(1), Word::fixnum(128)],
        ),
        Err(ObjectError::Unsupported)
    );

    ncl_ffi::deallocate_system_memory(address, 2).unwrap();
}

#[test]
fn tracked_sap_memory_round_trips_and_rejects_expired_addresses() {
    let (runtime, mut ctx) = fixture();
    let address = ncl_ffi::allocate_system_memory(16).unwrap();
    sap_set(
        &ctx,
        &runtime,
        &AlienType::Int,
        address,
        4,
        Word::fixnum(-42),
    )
    .unwrap();
    assert_eq!(
        sap_ref(&mut ctx, &runtime, &AlienType::Int, address, 4),
        Ok(Word::fixnum(-42))
    );
    assert!(matches!(
        sap_ref(
            &mut ctx,
            &runtime,
            &AlienType::Int,
            SystemAreaPointer::new(0x1000),
            0,
        ),
        Err(FfiError::ForeignMemory(
            ncl_sys::foreign::ForeignError::UnknownRegion
        ))
    ));
    ncl_ffi::deallocate_system_memory(address, 16).unwrap();
    assert!(matches!(
        sap_ref(&mut ctx, &runtime, &AlienType::Int, address, 4),
        Err(FfiError::ForeignMemory(
            ncl_sys::foreign::ForeignError::UnknownRegion
        ))
    ));
}

#[cfg(target_pointer_width = "64")]
#[test]
fn registered_sap_pointer_values_round_trip_full_width_addresses() {
    let (runtime, mut ctx) = fixture();
    ncl_ffi::register(&runtime).unwrap();
    let storage = ncl_ffi::allocate_system_memory(16).unwrap();
    let storage_word = storage.to_word(&mut ctx, &runtime).unwrap();
    let pointer = SystemAreaPointer::new(usize::MAX);
    let pointer_word = pointer.to_word(&mut ctx, &runtime).unwrap();

    call_builtin(
        &mut ctx,
        &runtime,
        "%SAP-SET-SAP",
        &[storage_word, Word::fixnum(0), pointer_word],
    )
    .unwrap();
    let first_read = call_builtin(
        &mut ctx,
        &runtime,
        "SAP-REF-SAP",
        &[storage_word, Word::fixnum(0)],
    )
    .unwrap();
    call_builtin(
        &mut ctx,
        &runtime,
        "%SAP-SET-SAP",
        &[storage_word, Word::fixnum(8), first_read],
    )
    .unwrap();
    let round_trip = call_builtin(
        &mut ctx,
        &runtime,
        "SAP-REF-SAP",
        &[storage_word, Word::fixnum(8)],
    )
    .unwrap();

    assert_eq!(
        SystemAreaPointer::from_word(&ctx, round_trip).unwrap(),
        pointer
    );
    ncl_ffi::deallocate_system_memory(storage, 16).unwrap();
}

#[test]
fn system_memory_release_checks_size_and_expires_the_sap() {
    let address = ncl_ffi::allocate_system_memory(8).unwrap();
    assert_eq!(
        ncl_ffi::deallocate_system_memory(address, 4),
        Err(FfiError::AllocationSizeMismatch {
            expected: 8,
            got: 4,
        })
    );
    ncl_ffi::deallocate_system_memory(address, 8).unwrap();
    assert_eq!(
        ncl_ffi::deallocate_system_memory(address, 8),
        Err(FfiError::ForeignMemory(
            ncl_sys::foreign::ForeignError::UnknownRegion
        ))
    );
}

#[test]
fn comparisons_follow_address_order() {
    let low = SystemAreaPointer::new(0x10);
    let high = SystemAreaPointer::new(0x20);
    assert!(sap_eq(low, SystemAreaPointer::new(0x10)));
    assert!(sap_lt(low, high));
    assert!(sap_le(low, low));
    assert!(sap_gt(high, low));
    assert!(sap_ge(high, high));
    assert!(!sap_lt(high, low));
}

fn call_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    with_rooted_slice(ctx, args, |ctx, args| {
        let function = runtime
            .function(ctx, "NCL-FFI", name)
            .ok_or(ObjectError::UndefinedFunction)?;
        let function = FunctionObject::try_from(function).map_err(|_| ObjectError::TypeError)?;
        runtime.call_builtin(ctx, function, args)
    })
}
