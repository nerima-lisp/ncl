use super::*;

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
    array_row_major_set(&mut ctx, array, 3, Word::fixnum(9))?;
    assert_eq!(array_row_major_ref(&ctx, array, 3)?, Word::fixnum(9));
    let adjusted = adjust_array(&mut ctx, &runtime, array, &[3, 2], Word::fixnum(7))?;
    assert_eq!(array_dimensions(&ctx, adjusted)?, vec![3, 2]);
    assert_eq!(array_row_major_ref(&ctx, adjusted, 0)?, Word::fixnum(0));
    assert_eq!(array_row_major_ref(&ctx, adjusted, 4)?, Word::fixnum(7));
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
    Ok(())
}
