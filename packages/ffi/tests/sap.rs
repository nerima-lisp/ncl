#![allow(clippy::unwrap_used, reason = "tests assert on concrete values")]

//! System area pointer arithmetic and comparisons.

use ncl_ffi::{
    AlienType, FfiError, SystemAreaPointer, sap_eq, sap_ge, sap_gt, sap_le, sap_lt, sap_ref,
    sap_set,
};
use ncl_object::{
    Bignum, DoubleFloat, FunctionObject, ObjectError, Runtime, ThreadContext, Word, bignum_limbs,
    bignum_sign, double_value, make_bignum_from_i128, make_bignum_from_limbs, make_double,
    with_rooted_slice,
};
use ncl_sys::foreign::ForeignError;

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

    let negative_bignum = make_bignum_from_i128(&mut ctx, &runtime, -i128::from(u64::MAX))
        .unwrap()
        .as_word();
    assert!(matches!(
        SystemAreaPointer::from_word(&ctx, negative_bignum),
        Err(FfiError::ValueOutOfRange { .. })
    ));

    let above_u128 = make_bignum_from_limbs(&mut ctx, &runtime, false, &[0, 0, 0, 0, 1])
        .unwrap()
        .as_word();
    assert!(matches!(
        SystemAreaPointer::from_word(&ctx, above_u128),
        Err(FfiError::ValueOutOfRange { .. })
    ));
}

#[cfg(target_pointer_width = "64")]
#[test]
fn sap_difference_encodes_fixnums_and_signed_bignums() {
    let (runtime, mut ctx) = fixture();
    ncl_ffi::register(&runtime).unwrap();

    assert_eq!(
        call_builtin(
            &mut ctx,
            &runtime,
            "SAP-",
            &[Word::fixnum(0x1000), Word::fixnum(0x1020)],
        )
        .unwrap(),
        Word::fixnum(-32)
    );

    let high = SystemAreaPointer::new(1_usize << 62)
        .to_word(&mut ctx, &runtime)
        .unwrap();
    let positive = call_builtin(&mut ctx, &runtime, "SAP-", &[high, Word::fixnum(0)]).unwrap();
    assert_bignum(&ctx, positive, false, &[0, 1 << 30]);

    let above_fixnum = SystemAreaPointer::new((1_usize << 62) + 1)
        .to_word(&mut ctx, &runtime)
        .unwrap();
    let negative =
        call_builtin(&mut ctx, &runtime, "SAP-", &[Word::fixnum(0), above_fixnum]).unwrap();
    assert_bignum(&ctx, negative, true, &[1, 1 << 30]);
}

fn assert_bignum(ctx: &ThreadContext, value: Word, negative: bool, limbs: &[u32]) {
    assert_eq!(value.as_fixnum(), None);
    let value = Bignum::from_word(value);
    assert_eq!(bignum_sign(ctx, value), Ok(negative));
    assert_eq!(bignum_limbs(ctx, value), Ok(limbs.to_vec()));
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
fn registered_sap_integer_accessors_cover_boundaries_and_word_aliases() {
    let (runtime, mut ctx) = fixture();
    ncl_ffi::register(&runtime).unwrap();
    let address = ncl_ffi::allocate_system_memory(56).unwrap();
    let mut address_word = address.to_word(&mut ctx, &runtime).unwrap();

    ncl_object::with_root(&mut ctx, &mut address_word, |ctx, address_word| {
        let base = *address_word;
        let cases = [
            (0, "%SAP-SET-U8", "SAP-REF-8", 0),
            (1, "%SAP-SET-U8", "SAP-REF-8", i64::from(u8::MAX)),
            (2, "%SAP-SET-I8", "SIGNED-SAP-REF-8", i64::from(i8::MIN)),
            (3, "%SAP-SET-I8", "SIGNED-SAP-REF-8", i64::from(i8::MAX)),
            (4, "%SAP-SET-U16", "SAP-REF-16", 0),
            (6, "%SAP-SET-U16", "SAP-REF-16", i64::from(u16::MAX)),
            (8, "%SAP-SET-I16", "SIGNED-SAP-REF-16", i64::from(i16::MIN)),
            (10, "%SAP-SET-I16", "SIGNED-SAP-REF-16", i64::from(i16::MAX)),
            (12, "%SAP-SET-U32", "SAP-REF-32", 0),
            (16, "%SAP-SET-U32", "SAP-REF-32", i64::from(u32::MAX)),
            (20, "%SAP-SET-I32", "SIGNED-SAP-REF-32", i64::from(i32::MIN)),
            (24, "%SAP-SET-I32", "SIGNED-SAP-REF-32", i64::from(i32::MAX)),
        ];

        for (offset, setter, accessor, integer) in cases {
            let offset = Word::fixnum(offset);
            let value = Word::fixnum(integer);
            assert_eq!(
                call_builtin(ctx, &runtime, setter, &[base, offset, value]),
                Ok(value)
            );
            assert_eq!(
                call_builtin(ctx, &runtime, accessor, &[base, offset]),
                Ok(value)
            );
        }

        let unsigned_max = make_bignum_from_limbs(ctx, &runtime, false, &[u32::MAX, u32::MAX])
            .unwrap()
            .as_word();
        call_builtin(
            ctx,
            &runtime,
            "%SAP-SET-U64",
            &[base, Word::fixnum(32), unsigned_max],
        )
        .unwrap();
        let unsigned_64 =
            call_builtin(ctx, &runtime, "SAP-REF-64", &[base, Word::fixnum(32)]).unwrap();
        assert_bignum(ctx, unsigned_64, false, &[u32::MAX, u32::MAX]);
        let unsigned_word =
            call_builtin(ctx, &runtime, "SAP-REF-WORD", &[base, Word::fixnum(32)]).unwrap();
        assert_bignum(ctx, unsigned_word, false, &[u32::MAX, u32::MAX]);

        let signed_min = make_bignum_from_i128(ctx, &runtime, i128::from(i64::MIN))
            .unwrap()
            .as_word();
        call_builtin(
            ctx,
            &runtime,
            "%SAP-SET-I64",
            &[base, Word::fixnum(40), signed_min],
        )
        .unwrap();
        let signed_64 = call_builtin(
            ctx,
            &runtime,
            "SIGNED-SAP-REF-64",
            &[base, Word::fixnum(40)],
        )
        .unwrap();
        assert_bignum(ctx, signed_64, true, &[0, 0x8000_0000]);
        let signed_word = call_builtin(
            ctx,
            &runtime,
            "SIGNED-SAP-REF-WORD",
            &[base, Word::fixnum(40)],
        )
        .unwrap();
        assert_bignum(ctx, signed_word, true, &[0, 0x8000_0000]);

        let signed_max = make_bignum_from_i128(ctx, &runtime, i128::from(i64::MAX))
            .unwrap()
            .as_word();
        call_builtin(
            ctx,
            &runtime,
            "%SAP-SET-I64",
            &[base, Word::fixnum(48), signed_max],
        )
        .unwrap();
        let signed_max_read = call_builtin(
            ctx,
            &runtime,
            "SIGNED-SAP-REF-WORD",
            &[base, Word::fixnum(48)],
        )
        .unwrap();
        assert_bignum(ctx, signed_max_read, false, &[u32::MAX, 0x7fff_ffff]);
        Ok(())
    })
    .unwrap();

    ncl_ffi::deallocate_system_memory(address, 56).unwrap();
}

#[test]
fn registered_sap_float_accessors_round_trip_single_and_double_values() {
    let (runtime, mut ctx) = fixture();
    ncl_ffi::register(&runtime).unwrap();
    let address = ncl_ffi::allocate_system_memory(16).unwrap();
    let mut address_word = address.to_word(&mut ctx, &runtime).unwrap();

    ncl_object::with_root(&mut ctx, &mut address_word, |ctx, address_word| {
        let base = *address_word;
        let single_source = make_double(ctx, &runtime, 1.5).unwrap().as_word();
        call_builtin(
            ctx,
            &runtime,
            "%SAP-SET-SINGLE",
            &[base, Word::fixnum(0), single_source],
        )
        .unwrap();
        let single =
            call_builtin(ctx, &runtime, "SAP-REF-SINGLE", &[base, Word::fixnum(0)]).unwrap();
        call_builtin(
            ctx,
            &runtime,
            "%SAP-SET-DOUBLE",
            &[base, Word::fixnum(8), single],
        )
        .unwrap();
        let widened_single =
            call_builtin(ctx, &runtime, "SAP-REF-DOUBLE", &[base, Word::fixnum(8)]).unwrap();
        assert_eq!(
            double_value(ctx, DoubleFloat::from_word(widened_single)),
            Ok(1.5)
        );

        let double_source = make_double(ctx, &runtime, -13.25).unwrap().as_word();
        call_builtin(
            ctx,
            &runtime,
            "%SAP-SET-DOUBLE",
            &[base, Word::fixnum(8), double_source],
        )
        .unwrap();
        let double =
            call_builtin(ctx, &runtime, "SAP-REF-DOUBLE", &[base, Word::fixnum(8)]).unwrap();
        assert_eq!(
            double_value(ctx, DoubleFloat::from_word(double)),
            Ok(-13.25)
        );
        Ok(())
    })
    .unwrap();

    ncl_ffi::deallocate_system_memory(address, 16).unwrap();
}

#[test]
fn sap_accessors_report_alignment_and_bounds_without_partial_writes() {
    let (runtime, mut ctx) = fixture();
    ncl_ffi::register(&runtime).unwrap();
    let address = ncl_ffi::allocate_system_memory(6).unwrap();
    let address_word = address.to_word(&mut ctx, &runtime).unwrap();

    for (offset, value) in [(0, 0x1122), (2, 0x3344), (4, 0xbeef)] {
        assert_eq!(
            sap_set(
                &ctx,
                &runtime,
                &AlienType::UnsignedShort,
                address,
                offset,
                Word::fixnum(value),
            ),
            Ok(())
        );
    }

    let out_of_bounds = FfiError::ForeignMemory(ForeignError::OutOfBounds);
    assert_eq!(
        sap_ref(&mut ctx, &runtime, &AlienType::UnsignedInt, address, 4,),
        Err(out_of_bounds.clone())
    );
    assert_eq!(
        sap_set(
            &ctx,
            &runtime,
            &AlienType::UnsignedInt,
            address,
            4,
            Word::fixnum(0xdead_beef),
        ),
        Err(out_of_bounds)
    );

    let misaligned = FfiError::ForeignMemory(ForeignError::Misaligned);
    assert_eq!(
        sap_ref(&mut ctx, &runtime, &AlienType::UnsignedShort, address, 1,),
        Err(misaligned.clone())
    );
    assert_eq!(
        sap_set(
            &ctx,
            &runtime,
            &AlienType::UnsignedShort,
            address,
            1,
            Word::fixnum(0xa5a5),
        ),
        Err(misaligned)
    );

    assert_eq!(
        call_builtin(
            &mut ctx,
            &runtime,
            "SAP-REF-32",
            &[address_word, Word::fixnum(4)],
        ),
        Err(ObjectError::Unsupported)
    );
    assert_eq!(
        call_builtin(
            &mut ctx,
            &runtime,
            "%SAP-SET-U32",
            &[address_word, Word::fixnum(4), Word::fixnum(0xdead_beef),],
        ),
        Err(ObjectError::Unsupported)
    );

    for (offset, value) in [(0, 0x1122), (2, 0x3344), (4, 0xbeef)] {
        assert_eq!(
            sap_ref(
                &mut ctx,
                &runtime,
                &AlienType::UnsignedShort,
                address,
                offset,
            ),
            Ok(Word::fixnum(value))
        );
    }

    ncl_ffi::deallocate_system_memory(address, 6).unwrap();
}

#[test]
fn registered_sap_place_expanders_preserve_operands_and_accessor_forms() {
    let (runtime, mut ctx) = fixture();
    ncl_ffi::register(&runtime).unwrap();
    let ffi_package = runtime.ensure_package(&mut ctx, "NCL-FFI").unwrap();
    let ffi_package = ncl_object::Package::from_word(ffi_package);
    let base_form = Word::fixnum(0x1234);
    let offset_form = Word::fixnum(6);

    let accessors = [
        ("SAP-REF-8", "%SAP-SET-U8"),
        ("SIGNED-SAP-REF-8", "%SAP-SET-I8"),
        ("SAP-REF-16", "%SAP-SET-U16"),
        ("SIGNED-SAP-REF-16", "%SAP-SET-I16"),
        ("SAP-REF-32", "%SAP-SET-U32"),
        ("SIGNED-SAP-REF-32", "%SAP-SET-I32"),
        ("SAP-REF-64", "%SAP-SET-U64"),
        ("SIGNED-SAP-REF-64", "%SAP-SET-I64"),
        ("SAP-REF-WORD", "%SAP-SET-U64"),
        ("SIGNED-SAP-REF-WORD", "%SAP-SET-I64"),
        ("SAP-REF-SAP", "%SAP-SET-SAP"),
        ("SAP-REF-SINGLE", "%SAP-SET-SINGLE"),
        ("SAP-REF-DOUBLE", "%SAP-SET-DOUBLE"),
    ];

    for (accessor_name, setter_name) in accessors {
        let (accessor, _) = ffi_package
            .intern(&mut ctx, &runtime, accessor_name)
            .unwrap();
        let (setter, _) = ffi_package.intern(&mut ctx, &runtime, setter_name).unwrap();
        let expander = runtime.place_expander(&ctx, accessor).unwrap().unwrap();

        assert_eq!(
            expander(&mut ctx, &runtime, &[base_form]),
            Err(ObjectError::TypeError)
        );
        let expansion = expander(&mut ctx, &runtime, &[base_form, offset_form]).unwrap();

        assert_eq!(expansion.temporary_variables.len(), 2);
        assert_eq!(expansion.value_forms, vec![base_form, offset_form]);
        assert_eq!(expansion.store_variables.len(), 1);
        let base_temporary = expansion.temporary_variables.first().copied().unwrap();
        let offset_temporary = expansion.temporary_variables.get(1).copied().unwrap();
        let store_variable = expansion.store_variables.first().copied().unwrap();
        assert_ne!(base_temporary, offset_temporary);
        assert_ne!(store_variable, base_temporary);
        assert_ne!(store_variable, offset_temporary);
        assert_eq!(
            list_elements(&ctx, expansion.access_form),
            vec![accessor, base_temporary, offset_temporary]
        );
        assert_eq!(
            list_elements(&ctx, expansion.store_form),
            vec![setter, base_temporary, offset_temporary, store_variable]
        );
    }
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

fn list_elements(ctx: &ThreadContext, mut list: Word) -> Vec<Word> {
    let mut elements = Vec::new();
    while list != Word::NIL {
        elements.push(ncl_object::car(ctx, list).unwrap());
        list = ncl_object::cdr(ctx, list).unwrap();
    }
    elements
}
