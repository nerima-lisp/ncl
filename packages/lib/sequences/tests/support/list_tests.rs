use super::*;

#[test]
fn empty_list_sequence_has_no_elements() {
    let runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("Runtime::new failed: {error:?}"),
    };
    let mut ctx = ThreadContext::new();
    if let Err(error) = ctx.register(&runtime) {
        panic!("ThreadContext::register failed: {error:?}");
    }
    let sequence = Sequence::List(List::Nil);

    let length = match sequence_length(&mut ctx, sequence) {
        Ok(length) => length,
        Err(error) => panic!("sequence_length failed: {error:?}"),
    };
    assert_eq!(length, 0);
    assert!(sequence_elt(&mut ctx, sequence, Word::fixnum(0)).is_err());
}

#[test]
fn string_sequence_conversion_validates_word_character() {
    let runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("Runtime::new failed: {error:?}"),
    };
    let mut ctx = ThreadContext::new();
    if let Err(error) = ctx.register(&runtime) {
        panic!("ThreadContext::register failed: {error:?}");
    }
    let marker = Sequence::String(ncl_object::StringObject::from_word(Word::NIL));
    let string = match sequence_result(
        &mut ctx,
        &runtime,
        marker,
        &[Word::character(u32::from('λ'))],
    ) {
        Ok(string) => string,
        Err(error) => panic!("sequence_result failed: {error:?}"),
    };
    let character = match ncl_object::string_ref(&ctx, string, 0) {
        Ok(character) => character,
        Err(error) => panic!("string_ref failed: {error:?}"),
    };
    assert_eq!(character, 'λ');
    assert!(sequence_result(&mut ctx, &runtime, marker, &[Word::fixnum(65)]).is_err());
}
