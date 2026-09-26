use ncl_object::{
    List, ObjectError, ObjectRef, Runtime, Sequence, ThreadContext, Word, classify_object,
    make_cons,
};

pub fn sequence_value(ctx: &ThreadContext, word: Word) -> Result<Sequence, ObjectError> {
    if word == Word::NIL {
        return Ok(Sequence::List(List::Nil));
    }
    if word.is_cons() {
        return Ok(Sequence::List(List::Cons(ncl_object::Cons::from_word(
            word,
        ))));
    }
    match classify_object(ctx, word) {
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
    let mut values = values.to_vec();
    let value_tokens = values
        .iter_mut()
        .map(|value| ncl_object::push_root(ctx, value))
        .collect::<Vec<_>>();
    let mut result = Word::NIL;
    let result_token = ncl_object::push_root(ctx, &mut result);
    for &value in values.iter().rev() {
        match make_cons(ctx, runtime, value, result) {
            Ok(next) => result = next,
            Err(error) => {
                ncl_object::pop_root(ctx, result_token);
                for token in value_tokens.into_iter().rev() {
                    ncl_object::pop_root(ctx, token);
                }
                return Err(error);
            }
        }
    }
    let result_root_error =
        (!ncl_object::pop_root(ctx, result_token)).then_some(ObjectError::Layout);
    let value_root_error = value_tokens
        .into_iter()
        .rev()
        .find_map(|token| (!ncl_object::pop_root(ctx, token)).then_some(ObjectError::Layout));
    result_root_error
        .or(value_root_error)
        .map_or(Ok(result), Err)
}

pub mod filter;
pub mod list;
