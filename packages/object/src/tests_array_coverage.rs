use super::{ArrayElementType, ArrayOptions, length, make_array, read, write};
use crate::{Runtime, ThreadContext, make_simple_vector, make_string};
use ncl_sys::Word;

#[test]
fn element_type_decoding_and_low_level_array_access_are_explicit() {
    for (value, expected) in [
        (0, ArrayElementType::T),
        (1, ArrayElementType::Bit),
        (2, ArrayElementType::Character),
        (3, ArrayElementType::BaseChar),
        (4, ArrayElementType::Fixnum),
        (5, ArrayElementType::Signed),
        (6, ArrayElementType::Unsigned),
        (7, ArrayElementType::SingleFloat),
        (8, ArrayElementType::DoubleFloat),
    ] {
        assert_eq!(
            ArrayElementType::from_word(Word::fixnum(value)),
            Ok(expected)
        );
    }
    assert_eq!(
        ArrayElementType::from_word(Word::fixnum(9)),
        Err(crate::ObjectError::Layout)
    );
    assert_eq!(
        ArrayElementType::from_word(Word::TRUE),
        Err(crate::ObjectError::Layout)
    );

    let runtime_result = Runtime::new();
    assert!(runtime_result.is_ok());
    let Ok(runtime) = runtime_result else { return };
    let mut context = ThreadContext::new();
    let register_result = context.register(&runtime);
    assert!(register_result.is_ok());
    if register_result.is_err() {
        return;
    }
    let vector =
        make_simple_vector(&mut context, &runtime, &[Word::fixnum(3)]).unwrap_or(Word::NIL);
    assert_eq!(
        length(&context, vector, crate::widetag::SIMPLE_VECTOR, 0),
        Ok(1)
    );
    assert_eq!(
        read(&context, vector, 1, crate::widetag::SIMPLE_VECTOR),
        Ok(Word::fixnum(3))
    );
    assert!(
        write(
            &mut context,
            vector,
            1,
            Word::fixnum(4),
            crate::widetag::SIMPLE_VECTOR
        )
        .is_ok()
    );
    assert_eq!(
        read(&context, vector, 1, crate::widetag::STRING),
        Err(crate::ObjectError::TypeError)
    );
    assert_eq!(
        length(&context, Word::NIL, crate::widetag::SIMPLE_VECTOR, 0),
        Err(crate::ObjectError::TypeError)
    );
    let string = make_string(&mut context, &runtime, &['x']).unwrap_or(Word::NIL);
    assert_eq!(
        read(&context, string, 1, crate::widetag::STRING),
        Ok(Word::character('x' as u32))
    );
    assert_eq!(
        write(&mut context, string, 2, Word::NIL, crate::widetag::STRING),
        Err(crate::ObjectError::Storage(
            ncl_sys::StorageCondition::ThreadNotRegistered
        ))
    );
}

#[test]
fn adjustable_vector_operations_preserve_values_under_gc_stress() {
    let runtime_result = Runtime::new();
    assert!(runtime_result.is_ok());
    let Ok(runtime) = runtime_result else { return };
    let mut context = ThreadContext::new();
    let register_result = context.register(&runtime);
    assert!(register_result.is_ok());
    if register_result.is_err() {
        return;
    }
    context.set_gc_stress(true);
    context.set_strict_forwarding(true);

    let vector_result = make_array(
        &mut context,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: true,
            fill_pointer: Some(1),
            displaced_to: None,
            displaced_index_offset: 0,
        },
    );
    assert!(vector_result.is_ok());
    let Ok(vector) = vector_result else { return };

    assert_eq!(
        crate::array::vector_push(&mut context, vector, Word::fixnum(1)),
        Ok(None)
    );
    let extended_result =
        crate::array::vector_push_extend(&mut context, &runtime, vector, Word::fixnum(7), 2);
    assert!(extended_result.is_ok());
    let Ok((index, extended)) = extended_result else {
        return;
    };
    assert_eq!(index, 1);
    assert_eq!(
        crate::array::array_row_major_ref(&context, extended, 1),
        Ok(Word::fixnum(7))
    );
    assert_eq!(
        crate::array::vector_pop(&mut context, extended),
        Ok(Word::fixnum(7))
    );
    assert_eq!(
        crate::array::vector_pop(&mut context, extended),
        Ok(Word::NIL)
    );
    assert_eq!(
        crate::array::vector_pop(&mut context, extended),
        Err(crate::ObjectError::TypeError)
    );
    assert_eq!(
        crate::array::vector_push_extend(&mut context, &runtime, extended, Word::NIL, 0),
        Err(crate::ObjectError::TypeError)
    );
}

#[test]
fn array_displacement_and_element_validation_report_observable_results() {
    let runtime_result = Runtime::new();
    assert!(runtime_result.is_ok());
    let Ok(runtime) = runtime_result else { return };
    let mut context = ThreadContext::new();
    let register_result = context.register(&runtime);
    assert!(register_result.is_ok());
    if register_result.is_err() {
        return;
    }
    context.set_gc_stress(true);
    context.set_strict_forwarding(true);

    let target_result =
        make_simple_vector(&mut context, &runtime, &[Word::fixnum(2), Word::fixnum(3)]);
    assert!(target_result.is_ok());
    let Ok(mut target) = target_result else {
        return;
    };
    let target_token = crate::push_root(&mut context, &mut target);
    let displaced_result = make_array(
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
    );
    assert!(displaced_result.is_ok());
    let Ok(displaced) = displaced_result else {
        return;
    };
    assert_eq!(
        crate::array::array_displacement(&context, displaced),
        Ok((target, 1))
    );
    assert_eq!(
        crate::array::array_row_major_ref(&context, displaced, 0),
        Ok(Word::fixnum(3))
    );
    let set_result = crate::array::array_row_major_set(&mut context, displaced, 0, Word::fixnum(9));
    assert!(set_result.is_ok());
    if set_result.is_err() {
        return;
    }
    assert_eq!(
        crate::array::simple_vector_ref(&context, target, 1),
        Ok(Word::fixnum(9))
    );
    let fixnum_array_result = make_array(
        &mut context,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::Fixnum,
            initial_element: Word::fixnum(0),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    );
    assert!(fixnum_array_result.is_ok());
    let Ok(fixnum_array) = fixnum_array_result else {
        return;
    };
    assert_eq!(
        crate::array::array_row_major_set(&mut context, fixnum_array, 0, Word::TRUE),
        Err(crate::ObjectError::TypeError)
    );
    assert_eq!(
        crate::array::array_row_major_ref(&context, displaced, 1),
        Err(crate::ObjectError::TypeError)
    );
    assert!(crate::pop_root(&mut context, target_token));
}
