//! Array behavior and GC cases moved from kernel integration tests.

use ncl_arrays::{
    ArrayElementType, ArrayOptions, array_dimensions, array_row_major_ref, array_row_major_set,
    make_array, specialized_array_element_type,
};
use ncl_object::Word;
use ncl_object::{
    ObjectError, Runtime, ThreadContext, make_bignum_from_i128, make_simple_vector, make_string,
    specialized_array_ref, specialized_array_set, string_ref,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ncl_arrays::register_layouts(&runtime)
        .unwrap_or_else(|error| panic!("array layouts: {error:?}"));
    (runtime, context)
}

#[test]
fn specialized_and_displaced_arrays_cover_element_validation() {
    let (runtime, mut context) = setup();
    let cases = [
        (ArrayElementType::Bit, Word::fixnum(1)),
        (ArrayElementType::Character, Word::character(65)),
        (ArrayElementType::BaseChar, Word::character(66)),
        (ArrayElementType::Fixnum, Word::fixnum(3)),
        (ArrayElementType::Signed, Word::fixnum(-3)),
        (ArrayElementType::Unsigned, Word::fixnum(3)),
        (ArrayElementType::SingleFloat, Word::TRUE),
        (ArrayElementType::DoubleFloat, Word::TRUE),
    ];
    for (element_type, value) in cases {
        let array =
            ncl_object::make_specialized_array(&mut context, &runtime, element_type, &[value])
                .unwrap_or_else(|error| panic!("specialized array: {error:?}"));
        assert_eq!(
            specialized_array_element_type(&context, array),
            Ok(element_type)
        );
        assert_eq!(specialized_array_ref(&context, array, 0), Ok(value));
        assert!(specialized_array_set(&mut context, array, 0, value).is_ok());
        assert_eq!(
            specialized_array_ref(&context, array, 1),
            Err(ObjectError::TypeError)
        );
    }
    assert_eq!(
        ncl_object::make_specialized_array(&mut context, &runtime, ArrayElementType::T, &[]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ncl_object::make_specialized_array(
            &mut context,
            &runtime,
            ArrayElementType::Bit,
            &[Word::fixnum(2)]
        ),
        Err(ObjectError::TypeError)
    );
    let target = ncl_object::make_specialized_array(
        &mut context,
        &runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(7), Word::fixnum(8)],
    )
    .unwrap_or(Word::NIL);
    let displaced = make_array(
        &mut context,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: Some(target),
            displaced_index_offset: 1,
        },
    )
    .unwrap_or(Word::NIL);
    assert_eq!(
        array_row_major_ref(&context, displaced, 0),
        Ok(Word::fixnum(8))
    );
    assert!(array_row_major_set(&mut context, displaced, 0, Word::fixnum(11)).is_ok());
    assert_eq!(
        specialized_array_ref(&context, target, 1),
        Ok(Word::fixnum(11))
    );
    assert_eq!(
        array_dimensions(&context, Word::NIL),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        array_row_major_ref(&context, displaced, 1),
        Err(ObjectError::TypeError)
    );
    let malformed = ncl_object::allocate(
        &mut context,
        &runtime,
        ncl_object::layout::widetag::SPECIALIZED_ARRAY,
        3,
    )
    .unwrap_or(Word::NIL);
    assert!(ncl_object::put(&mut context, malformed, 0, Word::fixnum(99)).is_ok());
    assert!(ncl_object::put(&mut context, malformed, 1, Word::fixnum(1)).is_ok());
    assert!(ncl_object::put(&mut context, malformed, 2, Word::fixnum(1)).is_ok());
    assert_eq!(
        specialized_array_element_type(&context, malformed),
        Err(ObjectError::Layout)
    );
    assert_eq!(
        specialized_array_ref(&context, malformed, 0),
        Err(ObjectError::Layout)
    );
}

#[test]
fn signed_and_unsigned_arrays_validate_bignum_elements() {
    let (runtime, mut context) = setup();
    let negative = make_bignum_from_i128(&mut context, &runtime, -(1_i128 << 70))
        .unwrap_or_else(|error| panic!("negative bignum: {error:?}"))
        .into();
    let positive = make_bignum_from_i128(&mut context, &runtime, 1_i128 << 70)
        .unwrap_or_else(|error| panic!("positive bignum: {error:?}"))
        .into();
    let signed = ncl_object::make_specialized_array(
        &mut context,
        &runtime,
        ArrayElementType::Signed,
        &[negative],
    )
    .unwrap_or_else(|error| panic!("signed array: {error:?}"));
    assert_eq!(specialized_array_ref(&context, signed, 0), Ok(negative));
    assert!(specialized_array_set(&mut context, signed, 0, positive).is_ok());
    assert_eq!(specialized_array_ref(&context, signed, 0), Ok(positive));

    let unsigned = ncl_object::make_specialized_array(
        &mut context,
        &runtime,
        ArrayElementType::Unsigned,
        &[positive],
    )
    .unwrap_or_else(|error| panic!("unsigned array: {error:?}"));
    assert_eq!(specialized_array_ref(&context, unsigned, 0), Ok(positive));
    assert_eq!(
        specialized_array_set(&mut context, unsigned, 0, negative),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        specialized_array_set(&mut context, unsigned, 0, Word::fixnum(-1)),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ncl_object::make_specialized_array(
            &mut context,
            &runtime,
            ArrayElementType::Fixnum,
            &[positive],
        ),
        Err(ObjectError::TypeError)
    );

    let options = |element_type, initial_element| ArrayOptions {
        element_type,
        initial_element,
        adjustable: false,
        fill_pointer: None,
        displaced_to: None,
        displaced_index_offset: 0,
    };
    let general_signed = make_array(
        &mut context,
        &runtime,
        &[1],
        options(ArrayElementType::Signed, negative),
    )
    .unwrap_or_else(|error| panic!("general signed array: {error:?}"));
    assert_eq!(
        array_row_major_ref(&context, general_signed, 0),
        Ok(negative)
    );
    assert!(array_row_major_set(&mut context, general_signed, 0, positive).is_ok());
    assert_eq!(
        array_row_major_ref(&context, general_signed, 0),
        Ok(positive)
    );

    let general_unsigned = make_array(
        &mut context,
        &runtime,
        &[1],
        options(ArrayElementType::Unsigned, positive),
    )
    .unwrap_or_else(|error| panic!("general unsigned array: {error:?}"));
    assert_eq!(
        array_row_major_ref(&context, general_unsigned, 0),
        Ok(positive)
    );
    assert_eq!(
        array_row_major_set(&mut context, general_unsigned, 0, negative),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        array_row_major_set(&mut context, general_unsigned, 0, Word::fixnum(-1)),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn arrays_reject_invalid_options_and_displacement_targets() {
    let (runtime, mut context) = setup();
    let options = |fill_pointer, displaced_to, offset| ArrayOptions {
        element_type: ArrayElementType::T,
        initial_element: Word::NIL,
        adjustable: false,
        fill_pointer,
        displaced_to,
        displaced_index_offset: offset,
    };
    assert_eq!(
        make_array(&mut context, &runtime, &[1], options(Some(2), None, 0)),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        make_array(&mut context, &runtime, &[1, 1], options(Some(1), None, 0)),
        Err(ObjectError::TypeError)
    );
    assert!(
        make_array(
            &mut context,
            &runtime,
            &[2],
            options(None, Some(Word::TRUE), 0)
        )
        .is_ok()
    );
    let empty =
        make_array(&mut context, &runtime, &[0], options(None, None, 0)).unwrap_or(Word::NIL);
    assert_eq!(
        array_row_major_ref(&context, empty, 0),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        array_row_major_set(&mut context, empty, 0, Word::NIL),
        Err(ObjectError::TypeError)
    );
    let vector =
        make_simple_vector(&mut context, &runtime, &[Word::fixnum(1)]).unwrap_or(Word::NIL);
    let displaced = make_array(&mut context, &runtime, &[1], options(None, Some(vector), 1))
        .unwrap_or(Word::NIL);
    assert_eq!(
        array_row_major_ref(&context, displaced, 0),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        array_row_major_set(&mut context, displaced, 0, Word::NIL),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn nested_displacement_and_structure_metadata_have_observable_results() {
    let (runtime, mut context) = setup();
    let base = make_array(
        &mut context,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(3),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap_or(Word::NIL);
    let outer = make_array(
        &mut context,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: Some(base),
            displaced_index_offset: 1,
        },
    )
    .unwrap_or(Word::NIL);
    let nested = make_array(
        &mut context,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: Some(outer),
            displaced_index_offset: 0,
        },
    )
    .unwrap_or(Word::NIL);
    assert_eq!(array_dimensions(&context, nested), Ok(vec![1]));
    assert_eq!(
        array_row_major_ref(&context, nested, 0),
        Ok(Word::fixnum(3))
    );
    assert!(array_row_major_set(&mut context, nested, 0, Word::fixnum(8)).is_ok());
    assert_eq!(array_row_major_ref(&context, base, 1), Ok(Word::fixnum(8)));
}

#[test]
fn specialized_arrays_validate_element_type() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let array = ncl_object::make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1)],
    )
    .unwrap_or(Word::NIL);
    assert_eq!(specialized_array_ref(&ctx, array, 0), Ok(Word::fixnum(1)));
    assert!(specialized_array_set(&mut ctx, array, 0, Word::fixnum(2)).is_err());
    assert_eq!(
        ncl_object::classify_object(&ctx, array),
        ncl_object::ObjectRef::SpecializedArray(array)
    );
}

#[test]
fn non_simple_arrays_store_dimensions_and_row_major_values() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let array = make_array(
        &mut ctx,
        &runtime,
        &[2, 2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: true,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap_or(Word::NIL);
    assert_eq!(array_dimensions(&ctx, array), Ok(vec![2, 2]));
    assert!(array_row_major_set(&mut ctx, array, 3, Word::fixnum(8)).is_ok());
    assert_eq!(array_row_major_ref(&ctx, array, 3), Ok(Word::fixnum(8)));
    assert_eq!(
        ncl_object::classify_object(&ctx, array),
        ncl_object::ObjectRef::Array(array)
    );
}

#[test]
fn non_simple_array_references_survive_minor_and_full_gc() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    assert!(runtime.register_layouts().is_ok());
    ncl_arrays::register_layouts(&runtime)
        .unwrap_or_else(|error| panic!("array layouts: {error:?}"));
    let target = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(11)]).unwrap_or(Word::NIL);
    let rank_one = make_array(
        &mut ctx,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: Some(1),
            displaced_to: Some(target),
            displaced_index_offset: 0,
        },
    )
    .unwrap_or(Word::NIL);
    let payload = make_string(&mut ctx, &runtime, &['x']).unwrap_or(Word::NIL);
    let rank_three = make_array(
        &mut ctx,
        &runtime,
        &[1, 1, 1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: payload,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap_or(Word::NIL);
    let mut rank_one_root = rank_one;
    let _rank_one_token = ncl_object::push_root(&mut ctx, &mut rank_one_root);
    let mut rank_three_root = rank_three;
    let _rank_three_token = ncl_object::push_root(&mut ctx, &mut rank_three_root);
    assert!(ctx.collect(false).is_ok());
    assert!(ctx.collect(true).is_ok());
    assert_eq!(
        array_row_major_ref(&ctx, rank_one_root, 0),
        Ok(Word::fixnum(11))
    );
    let value = array_row_major_ref(&ctx, rank_three_root, 0).unwrap_or(Word::NIL);
    assert_eq!(string_ref(&ctx, value, 0), Ok('x'));
}
