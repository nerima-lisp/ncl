use super::{
    Alignment, ByteLength, ForeignCallError, ForeignError, ForeignFunction, ForeignRegion,
    ForeignSignature, ForeignType, ForeignValue, call,
};
use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};

static INVOCATIONS: AtomicUsize = AtomicUsize::new(0);

fn matches_f32(actual: f32, expected: f32) -> bool {
    actual.to_bits() == expected.to_bits()
}

fn matches_f64(actual: f64, expected: f64) -> bool {
    actual.to_bits() == expected.to_bits()
}

#[allow(
    clippy::too_many_arguments,
    reason = "the fixture exercises both ABI register banks and stack arguments"
)]
extern "C" fn mixed_stack_arguments(
    first_integer: u32,
    first_double: f64,
    first_pointer: *mut c_void,
    first_single: f32,
    second_integer: u32,
    second_double: f64,
    third_integer: u32,
    second_single: f32,
    fourth_integer: u32,
    third_double: f64,
    second_pointer: *mut c_void,
    third_single: f32,
    fifth_integer: u32,
    fourth_double: f64,
    fourth_single: f32,
    fifth_double: f64,
    sixth_integer: u32,
    fifth_single: f32,
    third_pointer: *mut c_void,
    sixth_double: f64,
) -> f64 {
    if first_integer == 1
        && matches_f64(first_double, 0.5)
        && !first_pointer.is_null()
        && matches_f32(first_single, 1.0)
        && second_integer == 2
        && matches_f64(second_double, 1.5)
        && third_integer == 3
        && matches_f32(second_single, 2.0)
        && fourth_integer == 4
        && matches_f64(third_double, 2.5)
        && !second_pointer.is_null()
        && matches_f32(third_single, 3.0)
        && fifth_integer == 5
        && matches_f64(fourth_double, 3.5)
        && matches_f32(fourth_single, 4.0)
        && matches_f64(fifth_double, 4.5)
        && sixth_integer == 6
        && matches_f32(fifth_single, 5.0)
        && !third_pointer.is_null()
        && matches_f64(sixth_double, 5.5)
    {
        740.0
    } else {
        -1.0
    }
}

#[cfg(target_arch = "x86_64")]
extern "C" fn return_stack_pointer(
    _a: u32,
    _b: u32,
    _c: u32,
    _d: u32,
    _e: u32,
    _f: u32,
    pointer: *mut c_void,
) -> *mut c_void {
    pointer
}

#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::too_many_arguments,
    reason = "the fixture places its pointer after all eight AAPCS64 GP registers"
)]
extern "C" fn return_stack_pointer(
    _a: u32,
    _b: u32,
    _c: u32,
    _d: u32,
    _e: u32,
    _f: u32,
    _g: u32,
    _h: u32,
    pointer: *mut c_void,
) -> *mut c_void {
    pointer
}

extern "C" fn add_f32(left: f32, right: f32) -> f32 {
    left + right
}

extern "C" fn should_not_run() -> u64 {
    INVOCATIONS.fetch_add(1, Ordering::SeqCst);
    1
}

fn foreign_function(address: *mut c_void) -> Result<ForeignFunction<'static>, ForeignError> {
    let address = NonNull::new(address).ok_or(ForeignError::NullAddress)?;
    Ok(ForeignFunction::from_test_address(address))
}

#[test]
fn mixed_integer_pointer_float_and_stack_arguments_use_sysv_or_aapcs64()
-> Result<(), Box<dyn std::error::Error>> {
    // check-added-lines: allow(as-cast) Cast the Rust `extern "C"` function pointer
    // to the raw address type used by `ForeignFunction`.
    let function = foreign_function(mixed_stack_arguments as *const () as *mut c_void)?;
    let signature = ForeignSignature::new(
        ForeignType::F64,
        vec![
            ForeignType::U32,
            ForeignType::F64,
            ForeignType::Pointer,
            ForeignType::F32,
            ForeignType::U32,
            ForeignType::F64,
            ForeignType::U32,
            ForeignType::F32,
            ForeignType::U32,
            ForeignType::F64,
            ForeignType::Pointer,
            ForeignType::F32,
            ForeignType::U32,
            ForeignType::F64,
            ForeignType::F32,
            ForeignType::F64,
            ForeignType::U32,
            ForeignType::F32,
            ForeignType::Pointer,
            ForeignType::F64,
        ],
    );
    let region = ForeignRegion::allocate(ByteLength::new(1), Alignment::ONE)?;
    let pointer = region.address();
    let values = [
        ForeignValue::U32(1),
        ForeignValue::F64(0.5),
        ForeignValue::Pointer(pointer),
        ForeignValue::F32(1.0),
        ForeignValue::U32(2),
        ForeignValue::F64(1.5),
        ForeignValue::U32(3),
        ForeignValue::F32(2.0),
        ForeignValue::U32(4),
        ForeignValue::F64(2.5),
        ForeignValue::Pointer(pointer),
        ForeignValue::F32(3.0),
        ForeignValue::U32(5),
        ForeignValue::F64(3.5),
        ForeignValue::F32(4.0),
        ForeignValue::F64(4.5),
        ForeignValue::U32(6),
        ForeignValue::F32(5.0),
        ForeignValue::Pointer(pointer),
        ForeignValue::F64(5.5),
    ];
    assert_eq!(
        // SAFETY: the signature matches `mixed_stack_arguments`; its pointers stay
        // live through the call, and the fixture does not dereference or retain them.
        unsafe { call(&function, &signature, &values) },
        Ok(ForeignValue::F64(740.0))
    );
    Ok(())
}

#[test]
fn single_precision_return_uses_the_float_result_register() -> Result<(), Box<dyn std::error::Error>>
{
    // check-added-lines: allow(as-cast) Cast the Rust `extern "C"` function pointer
    // to the raw address type used by `ForeignFunction`.
    let function = foreign_function(add_f32 as *const () as *mut c_void)?;
    let signature = ForeignSignature::new(ForeignType::F32, vec![ForeignType::F32; 2]);
    assert_eq!(
        // SAFETY: `add_f32` has this C ABI and does not unwind.
        unsafe {
            call(
                &function,
                &signature,
                &[ForeignValue::F32(1.25), ForeignValue::F32(2.5)],
            )
        },
        Ok(ForeignValue::F32(3.75))
    );
    Ok(())
}

#[test]
fn stack_pointer_return_preserves_the_pointer_value() -> Result<(), Box<dyn std::error::Error>> {
    // check-added-lines: allow(as-cast) Cast the Rust `extern "C"` function pointer
    // to the raw address type used by `ForeignFunction`.
    let function = foreign_function(return_stack_pointer as *const () as *mut c_void)?;
    #[cfg(target_arch = "x86_64")]
    let register_arguments = 6;
    #[cfg(target_arch = "aarch64")]
    let register_arguments = 8;
    let mut types = vec![ForeignType::U32; register_arguments];
    types.push(ForeignType::Pointer);
    let signature = ForeignSignature::new(ForeignType::Pointer, types);
    let mut values = Vec::new();
    for value in 0..register_arguments {
        values.push(ForeignValue::U32(u32::try_from(value)?));
    }
    let region = ForeignRegion::allocate(ByteLength::new(8), Alignment::ONE)?;
    let pointer = region.address();
    values.push(ForeignValue::Pointer(pointer));
    assert_eq!(
        // SAFETY: `return_stack_pointer` matches the signature, and the tracked
        // pointer remains live; the fixture only returns it without dereferencing.
        unsafe { call(&function, &signature, &values) },
        Ok(ForeignValue::Pointer(pointer))
    );
    Ok(())
}

#[test]
fn variadic_and_aggregate_signatures_are_rejected_before_invocation()
-> Result<(), Box<dyn std::error::Error>> {
    // check-added-lines: allow(as-cast) Cast the Rust `extern "C"` function pointer
    // to the raw address type used by `ForeignFunction`.
    let function = foreign_function(should_not_run as *const () as *mut c_void)?;
    INVOCATIONS.store(0, Ordering::SeqCst);

    let variadic = ForeignSignature::variadic(ForeignType::U64, Vec::new());
    assert_eq!(
        // SAFETY: the variadic-signature check rejects this before invoking the function.
        unsafe { call(&function, &variadic, &[]) },
        Err(ForeignCallError::UnsupportedAbi)
    );

    let aggregate_result = ForeignSignature::aggregate(ForeignType::Void, Vec::new());
    assert_eq!(
        // SAFETY: the aggregate-signature check rejects this before invocation.
        unsafe { call(&function, &aggregate_result, &[]) },
        Err(ForeignCallError::UnsupportedAbi)
    );

    let aggregate_argument = ForeignSignature::aggregate(ForeignType::U64, vec![ForeignType::U32]);
    assert_eq!(
        // SAFETY: the aggregate-signature check rejects this before invocation.
        unsafe { call(&function, &aggregate_argument, &[]) },
        Err(ForeignCallError::UnsupportedAbi)
    );
    assert_eq!(INVOCATIONS.load(Ordering::SeqCst), 0);

    let void_argument = ForeignSignature::new(ForeignType::U64, vec![ForeignType::Void]);
    assert_eq!(
        // SAFETY: the invalid-signature check rejects this before invocation.
        unsafe { call(&function, &void_argument, &[ForeignValue::Void]) },
        Err(ForeignCallError::InvalidSignature)
    );
    assert_eq!(INVOCATIONS.load(Ordering::SeqCst), 0);
    Ok(())
}
