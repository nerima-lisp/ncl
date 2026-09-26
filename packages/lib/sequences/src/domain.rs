use ncl_object::{
    List, ObjectError, ObjectRef, Runtime, Sequence, ThreadContext, Word, classify_object,
    make_cons,
};

pub fn sequence_value(ctx: &ThreadContext, word: Word) -> Result<Sequence, ObjectError> {
    if word == Word::NIL {
        return Ok(Sequence::List(List::Nil));
    }
    match classify_object(ctx, word) {
        ObjectRef::Cons(value) => Ok(Sequence::List(List::Cons(ncl_object::Cons::from_word(
            value,
        )))),
        ObjectRef::String(value) => {
            Ok(Sequence::String(ncl_object::StringObject::from_word(value)))
        }
        ObjectRef::SimpleVector(value) => {
            Ok(Sequence::Vector(ncl_object::SimpleVector::from_word(value)))
        }
        _ => Err(ObjectError::TypeError),
    }
}

pub fn list_from(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let mut result = Word::NIL;
        for value in roots.iter().rev() {
            let next = ncl_object::with_root(ctx, &mut result, |ctx, result| {
                make_cons(ctx, runtime, **value, *result)
            })?;
            result = next;
        }
        Ok(result)
    })
}

pub mod filter;
pub mod list;
