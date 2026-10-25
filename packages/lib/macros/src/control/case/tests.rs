use super::case;
use crate::{elements, list, symbol};
use ncl_object::{HeapRootedWord, HeapRootedWords, ObjectError, Runtime, ThreadContext, Word};

fn fixture() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    crate::register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    Ok((runtime, ctx))
}

#[test]
fn case_roots_multiple_keys_and_otherwise_clause_under_gc_stress() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let keys = list(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)])?;
    let keys = HeapRootedWord::new(&ctx, keys)?;
    let clause = list(&mut ctx, &runtime, &[keys.get(), Word::fixnum(10)])?;
    let clause = HeapRootedWord::new(&ctx, clause)?;
    let otherwise = symbol(&mut ctx, &runtime, "OTHERWISE")?;
    let otherwise = HeapRootedWord::new(&ctx, otherwise)?;
    let fallback = list(&mut ctx, &runtime, &[otherwise.get(), Word::fixnum(11)])?;
    let fallback = HeapRootedWord::new(&ctx, fallback)?;
    let values = HeapRootedWords::new(&ctx, &[Word::fixnum(99), clause.get(), fallback.get()])?;
    let let_operator = symbol(&mut ctx, &runtime, "LET")?;
    let let_operator = HeapRootedWord::new(&ctx, let_operator)?;

    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let expansion = case(&mut ctx, &runtime, values.as_slice(), false)?;
    let expansion = HeapRootedWord::new(&ctx, expansion)?;
    let parts = elements(&mut ctx, expansion.get())?;
    assert_eq!(parts.first().copied(), Some(let_operator.get()));
    Ok(())
}

#[test]
fn ecase_builds_a_type_error_for_unmatched_keys() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let first_key = symbol(&mut ctx, &runtime, "FIRST-KEY")?;
    let first_key = HeapRootedWord::new(&ctx, first_key)?;
    let second_key = symbol(&mut ctx, &runtime, "SECOND-KEY")?;
    let second_key = HeapRootedWord::new(&ctx, second_key)?;
    let body = symbol(&mut ctx, &runtime, "BODY-FORM")?;
    let body = HeapRootedWord::new(&ctx, body)?;
    let keys = list(&mut ctx, &runtime, &[first_key.get(), second_key.get()])?;
    let keys = HeapRootedWord::new(&ctx, keys)?;
    let clause = list(&mut ctx, &runtime, &[keys.get(), body.get()])?;
    let clause = HeapRootedWord::new(&ctx, clause)?;
    let value = symbol(&mut ctx, &runtime, "TEST-VALUE")?;
    let value = HeapRootedWord::new(&ctx, value)?;
    let values = HeapRootedWords::new(&ctx, &[value.get(), clause.get()])?;
    let let_operator = symbol(&mut ctx, &runtime, "LET")?;
    let let_operator = HeapRootedWord::new(&ctx, let_operator)?;
    let error_operator = symbol(&mut ctx, &runtime, "ERROR")?;
    let error_operator = HeapRootedWord::new(&ctx, error_operator)?;
    let member_operator = symbol(&mut ctx, &runtime, "MEMBER")?;
    let member_operator = HeapRootedWord::new(&ctx, member_operator)?;

    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let expansion = case(&mut ctx, &runtime, values.as_slice(), true)?;
    let expansion = HeapRootedWord::new(&ctx, expansion)?;
    let parts = elements(&mut ctx, expansion.get())?;
    assert_eq!(parts.first().copied(), Some(let_operator.get()));
    let branches = *parts.get(2).ok_or(ObjectError::TypeError)?;
    let branches = elements(&mut ctx, branches)?;
    let type_error = *branches.get(3).ok_or(ObjectError::TypeError)?;
    let type_error = elements(&mut ctx, type_error)?;
    assert_eq!(type_error.first().copied(), Some(error_operator.get()));
    let expected_type = *type_error.get(5).ok_or(ObjectError::TypeError)?;
    let expected_type = elements(&mut ctx, expected_type)?;
    let accepted_keys = *expected_type.get(1).ok_or(ObjectError::TypeError)?;
    let accepted_keys = elements(&mut ctx, accepted_keys)?;
    assert_eq!(
        accepted_keys,
        [member_operator.get(), first_key.get(), second_key.get()]
    );
    let body_form = *branches.get(2).ok_or(ObjectError::TypeError)?;
    let body_form = elements(&mut ctx, body_form)?;
    assert_eq!(body_form.get(1), Some(&body.get()));
    Ok(())
}

#[test]
fn case_rejects_a_clause_without_a_key() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let values = HeapRootedWords::new(&ctx, &[Word::fixnum(99), Word::NIL])?;
    assert_eq!(
        case(&mut ctx, &runtime, values.as_slice(), false),
        Err(ObjectError::TypeError)
    );
    Ok(())
}
