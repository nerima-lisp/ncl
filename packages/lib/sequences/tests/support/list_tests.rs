use super::*;

fn fixnum(word: Word) -> ncl_object::Fixnum {
    match ncl_object::Fixnum::try_from_word(word) {
        Ok(value) => value,
        Err(error) => panic!("expected fixnum word: {error:?}"),
    }
}

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

    let length = match sequence_length(&ctx, sequence) {
        Ok(length) => length,
        Err(error) => panic!("sequence_length failed: {error:?}"),
    };
    assert_eq!(length, 0);
    assert_eq!(
        sequence_elt(&ctx, sequence, Word::fixnum(0)),
        Err(ObjectError::TypeError)
    );
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
    assert_eq!(
        sequence_result(&mut ctx, &runtime, marker, &[Word::fixnum(65)]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn list_boundaries_return_nil_and_preserve_exact_nth_error() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let empty = List::Nil;

    assert_eq!(
        nth(&ctx, &runtime, fixnum(Word::fixnum(0)), empty),
        Ok(Word::NIL)
    );
    assert_eq!(
        nthcdr(&ctx, &runtime, fixnum(Word::fixnum(3)), empty),
        Ok(Word::NIL)
    );
    assert_eq!(
        nth(&ctx, &runtime, fixnum(Word::fixnum(-1)), empty),
        Err(LispError::TypeError {
            datum: Word::fixnum(-1),
            expected: ncl_object::ObjectType::Fixnum,
        })
    );
    Ok(())
}

#[test]
fn list_star_without_a_tail_reports_the_exact_type_error() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    assert_eq!(
        list_star(&mut ctx, &runtime, &[]),
        Err(LispError::TypeError {
            datum: Word::NIL,
            expected: ncl_object::ObjectType::Cons,
        })
    );
    Ok(())
}

#[test]
fn subseq_rejects_reversed_and_past_end_ranges() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let sequence = Sequence::List(List::Nil);

    assert_eq!(
        sequence_subseq(
            &mut ctx,
            &runtime,
            sequence,
            Word::fixnum(1),
            Some(Word::fixnum(0)),
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        sequence_subseq(
            &mut ctx,
            &runtime,
            sequence,
            Word::fixnum(0),
            Some(Word::fixnum(1)),
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}
