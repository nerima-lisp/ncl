use super::{
    List, ObjectError, ObjectRef, Runtime, Sequence, ThreadContext, Word, classify_object,
    scope_rooted_slice,
};
use ncl_object::make_cons;

#[allow(clippy::needless_pass_by_ref_mut)]
pub fn list_from_values(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &mut [Word], // check-added-lines: allow(index) slice type
) -> Result<Word, ObjectError> {
    scope_rooted_slice(ctx, values, |ctx, rooted_values| {
        let mut result = Word::NIL;
        for value in rooted_values.iter().rev().copied() {
            result = make_cons(ctx, runtime, value, result)?;
        }
        Ok(result)
    })
}

pub fn object_sequence(ctx: &ThreadContext, word: Word) -> Result<Sequence, ObjectError> {
    if word == Word::NIL {
        return Ok(Sequence::List(List::Nil));
    }
    if word.is_cons() {
        return Ok(Sequence::List(List::Cons(ncl_object::Cons::from_word(
            word,
        ))));
    }
    if let ObjectRef::String(value) = classify_object(ctx, word) {
        Ok(Sequence::String(ncl_object::StringObject::from_word(value)))
    } else if let ObjectRef::SimpleVector(value) = classify_object(ctx, word) {
        Ok(Sequence::Vector(ncl_object::SimpleVector::from_word(value)))
    } else {
        Err(ObjectError::TypeError)
    }
}
