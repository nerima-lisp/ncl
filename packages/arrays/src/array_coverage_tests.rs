use super::{ArrayElementType, length, read, write};
use crate::{Runtime, ThreadContext};
use ncl_object::{Word, make_simple_vector, make_string};

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
        length(&context, vector, ncl_object::layout::widetag::SIMPLE_VECTOR, 0),
        Ok(1)
    );
    assert_eq!(
        read(&context, vector, 1, ncl_object::layout::widetag::SIMPLE_VECTOR),
        Ok(Word::fixnum(3))
    );
    assert!(
        write(
            &mut context,
            vector,
            1,
            Word::fixnum(4),
            ncl_object::layout::widetag::SIMPLE_VECTOR
        )
        .is_ok()
    );
    assert_eq!(
        read(&context, vector, 1, ncl_object::layout::widetag::STRING),
        Err(crate::ObjectError::TypeError)
    );
    assert_eq!(
        length(&context, Word::NIL, ncl_object::layout::widetag::SIMPLE_VECTOR, 0),
        Err(crate::ObjectError::TypeError)
    );
    let string = make_string(&mut context, &runtime, &['x']).unwrap_or(Word::NIL);
    assert_eq!(
        read(&context, string, 1, ncl_object::layout::widetag::STRING),
        Ok(Word::character('x' as u32))
    );
    assert!(matches!(
        write(
            &mut context,
            string,
            2,
            Word::NIL,
            ncl_object::layout::widetag::STRING,
        ),
        Err(crate::ObjectError::Storage(_))
    ));
}
