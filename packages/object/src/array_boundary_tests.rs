#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::super::*;
    use crate::{make_simple_vector, make_specialized_array, make_string};

    #[test]
    fn array_family_boundaries_return_values() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut context = ThreadContext::new();
        context.register(&runtime)?;

        let vector =
            make_simple_vector(&mut context, &runtime, &[Word::fixnum(1), Word::fixnum(2)])?;
        assert_eq!(array_element_type(&context, vector)?, ArrayElementType::T);
        assert_eq!(array_dimensions(&context, vector)?, vec![2]);
        assert_eq!(array_displacement(&context, vector)?, (Word::NIL, 0));
        assert_eq!(array_row_major_ref(&context, vector, 1)?, Word::fixnum(2));
        assert_eq!(
            array_row_major_set(&mut context, vector, 1, Word::fixnum(7)),
            Ok(())
        );
        assert_eq!(array_row_major_ref(&context, vector, 1)?, Word::fixnum(7));
        assert_eq!(
            array_row_major_ref(&context, vector, 2),
            Err(ObjectError::TypeError)
        );

        let string = make_string(&mut context, &runtime, &['a', 'b'])?;
        assert_eq!(
            array_element_type(&context, string)?,
            ArrayElementType::Character
        );
        assert_eq!(array_dimensions(&context, string)?, vec![2]);
        assert_eq!(array_displacement(&context, string)?, (Word::NIL, 0));
        assert_eq!(
            array_row_major_ref(&context, string, 0)?,
            Word::character('a' as u32)
        );
        assert_eq!(
            array_row_major_set(&mut context, string, 0, Word::character('z' as u32)),
            Ok(())
        );
        assert_eq!(
            array_row_major_ref(&context, string, 0)?,
            Word::character('z' as u32)
        );

        let specialized = make_specialized_array(
            &mut context,
            &runtime,
            ArrayElementType::Fixnum,
            &[Word::fixnum(3)],
        )?;
        assert_eq!(
            array_element_type(&context, specialized)?,
            ArrayElementType::Fixnum
        );
        assert_eq!(array_dimensions(&context, specialized)?, vec![1]);
        assert_eq!(array_displacement(&context, specialized)?, (Word::NIL, 0));
        assert_eq!(
            array_row_major_ref(&context, specialized, 0)?,
            Word::fixnum(3)
        );
        assert_eq!(
            array_row_major_set(&mut context, specialized, 0, Word::fixnum(4)),
            Ok(())
        );
        assert_eq!(
            array_row_major_ref(&context, specialized, 0)?,
            Word::fixnum(4)
        );

        assert_eq!(
            array_element_type(&context, Word::NIL),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            array_dimensions(&context, Word::NIL),
            Err(ObjectError::TypeError)
        );
        Ok(())
    }

    #[test]
    fn array_metadata_accessors_cover_simple_and_adjustable_vectors() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register");
        let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).expect("vector");
        assert_eq!(array_element_type(&ctx, vector), Ok(ArrayElementType::T));
        assert_eq!(array_displacement(&ctx, vector), Ok((Word::NIL, 0)));
        assert_eq!(
            adjustable_array_p(&ctx, vector),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            array_has_fill_pointer_p(&ctx, vector),
            Err(ObjectError::TypeError)
        );
        assert_eq!(fill_pointer(&ctx, vector), Err(ObjectError::TypeError));

        let array = make_array(
            &mut ctx,
            &runtime,
            &[2],
            ArrayOptions {
                element_type: ArrayElementType::T,
                initial_element: Word::fixnum(4),
                adjustable: true,
                fill_pointer: Some(1),
                displaced_to: None,
                displaced_index_offset: 0,
            },
        )
        .expect("array");
        assert!(adjustable_array_p(&ctx, array).expect("adjustable"));
        assert!(array_has_fill_pointer_p(&ctx, array).expect("fill pointer"));
        assert_eq!(fill_pointer(&ctx, array), Ok(1));
        assert_eq!(set_fill_pointer(&mut ctx, array, 2), Ok(()));
        assert_eq!(fill_pointer(&ctx, array), Ok(2));
        assert_eq!(array_row_major_ref(&ctx, array, 0), Ok(Word::fixnum(4)));
        assert_eq!(
            array_row_major_set(&mut ctx, array, 1, Word::fixnum(8)),
            Ok(())
        );
        assert_eq!(array_row_major_ref(&ctx, array, 1), Ok(Word::fixnum(8)));
    }
}
