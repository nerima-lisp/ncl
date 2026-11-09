use super::*;
use ncl_object::make_simple_vector;

#[test]
fn multidimensional_displaced_and_adjusted_arrays_preserve_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let array = make_array(
        &mut ctx,
        &runtime,
        &[2, 2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(0),
            adjustable: true,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    assert_eq!(array_dimensions(&ctx, array)?, vec![2, 2]);
    assert_eq!(array_element_type(&ctx, array)?, ArrayElementType::T);
    assert_eq!(array_displacement(&ctx, array)?, (Word::NIL, 0));
    assert!(adjustable_array_p(&ctx, array)?);
    assert!(!array_has_fill_pointer_p(&ctx, array)?);
    assert_eq!(fill_pointer(&ctx, array), Err(ObjectError::TypeError));
    array_row_major_set(&mut ctx, array, 3, Word::fixnum(9))?;
    assert_eq!(array_row_major_ref(&ctx, array, 3)?, Word::fixnum(9));
    assert_eq!(
        array_row_major_ref(&ctx, array, 4),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        array_row_major_set(&mut ctx, array, 4, Word::fixnum(1)),
        Err(ObjectError::TypeError)
    );

    let adjusted = adjust_array(&mut ctx, &runtime, array, &[3, 2], Word::fixnum(7))?;
    assert_ne!(adjusted, array, "adjust-array returns a distinct array");
    assert_eq!(array_dimensions(&ctx, array)?, vec![2, 2]);
    assert_eq!(array_row_major_ref(&ctx, array, 3)?, Word::fixnum(9));
    assert_eq!(array_dimensions(&ctx, adjusted)?, vec![3, 2]);
    assert_eq!(array_row_major_ref(&ctx, adjusted, 0)?, Word::fixnum(0));
    assert_eq!(array_row_major_ref(&ctx, adjusted, 3)?, Word::fixnum(9));
    assert_eq!(array_row_major_ref(&ctx, adjusted, 4)?, Word::fixnum(7));
    assert_eq!(array_row_major_ref(&ctx, adjusted, 5)?, Word::fixnum(7));

    // CLHS ADJUST-ARRAY preserves old elements that remain in bounds.
    let reduced = adjust_array(&mut ctx, &runtime, adjusted, &[1, 2], Word::fixnum(6))?;
    assert_eq!(array_dimensions(&ctx, reduced)?, vec![1, 2]);
    assert_eq!(array_row_major_ref(&ctx, reduced, 0)?, Word::fixnum(0));
    assert_eq!(array_row_major_ref(&ctx, reduced, 1)?, Word::fixnum(0));
    assert_eq!(
        array_row_major_ref(&ctx, reduced, 2),
        Err(ObjectError::TypeError)
    );

    let target = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
    )?;
    let displaced = make_array(
        &mut ctx,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: Some(target),
            displaced_index_offset: 1,
        },
    )?;
    assert_eq!(array_displacement(&ctx, displaced)?, (target, 1));
    assert_eq!(array_row_major_ref(&ctx, displaced, 0)?, Word::fixnum(2));
    array_row_major_set(&mut ctx, displaced, 1, Word::fixnum(8))?;
    assert_eq!(simple_vector_ref(&ctx, target, 2)?, Word::fixnum(8));
    assert_eq!(
        set_fill_pointer(&mut ctx, displaced, 1),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

fn assert_adjust_array_preserves_fill_pointer(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    // CLHS ADJUST-ARRAY with a nil fill-pointer argument leaves the old pointer unchanged.
    let vector = make_array(
        ctx,
        runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(5),
            adjustable: true,
            fill_pointer: Some(1),
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let adjusted = adjust_array(ctx, runtime, vector, &[3], Word::fixnum(9))?;
    assert_eq!(array_dimensions(ctx, adjusted)?, vec![3]);
    assert_eq!(fill_pointer(ctx, adjusted), Ok(1));
    assert_eq!(array_row_major_ref(ctx, adjusted, 0)?, Word::fixnum(5));
    assert_eq!(array_row_major_ref(ctx, adjusted, 1)?, Word::fixnum(5));
    assert_eq!(array_row_major_ref(ctx, adjusted, 2)?, Word::fixnum(9));
    Ok(())
}

#[test]
fn vector_push_pop_and_invalid_array_operations_return_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let vector = make_array(
        &mut ctx,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: true,
            fill_pointer: Some(0),
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    assert!(adjustable_array_p(&ctx, vector)?);
    assert!(array_has_fill_pointer_p(&ctx, vector)?);
    assert_eq!(fill_pointer(&ctx, vector), Ok(0));
    assert_eq!(
        adjust_array(&mut ctx, &runtime, vector, &[1, 1], Word::NIL),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        set_fill_pointer(&mut ctx, vector, 5),
        Err(ObjectError::TypeError)
    );
    assert_eq!(vector_push(&mut ctx, vector, Word::fixnum(4))?, Some(0));
    assert_eq!(vector_pop(&mut ctx, vector)?, Word::fixnum(4));
    assert_eq!(vector_pop(&mut ctx, vector), Err(ObjectError::TypeError));
    assert_eq!(
        vector_push_extend(&mut ctx, &runtime, vector, Word::fixnum(5), 0),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        array_row_major_ref(&ctx, Word::NIL, 0),
        Err(ObjectError::TypeError)
    );

    let fixed = make_array(
        &mut ctx,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: Some(1),
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    assert_eq!(
        vector_push_extend(&mut ctx, &runtime, fixed, Word::fixnum(8), 1),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        adjust_array(&mut ctx, &runtime, fixed, &[2], Word::NIL),
        Err(ObjectError::TypeError)
    );

    let rank_zero = make_array(
        &mut ctx,
        &runtime,
        &[],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    write(
        &mut ctx,
        rank_zero,
        metadata_offset(0, 0),
        Word::fixnum(0),
        layout::widetag::NON_SIMPLE_ARRAY,
    )?;
    write(
        &mut ctx,
        rank_zero,
        metadata_offset(0, 3),
        Word::fixnum(
            i64::try_from(layout::array_offset::FLAG_HAS_FILL_POINTER)
                .map_err(|_| ObjectError::Layout)?,
        ),
        layout::widetag::NON_SIMPLE_ARRAY,
    )?;
    assert_eq!(array_dimensions(&ctx, rank_zero)?, Vec::<usize>::new());
    assert_eq!(vector_push(&mut ctx, rank_zero, Word::fixnum(1)), Ok(None));
    assert_eq!(
        vector_push_extend(&mut ctx, &runtime, rank_zero, Word::fixnum(1), 1),
        Err(ObjectError::TypeError)
    );
    assert_adjust_array_preserves_fill_pointer(&runtime, &mut ctx)?;
    Ok(())
}
