use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    BuiltinFunctionCaller, FunctionArguments, FunctionCaller, LispError, List, MultipleValues,
    ObjectError, ObjectRef, Runtime, Sequence, ThreadContext, Word, classify_object, make_cons,
};

pub fn list_word(list: List) -> Word {
    match list {
        List::Nil => Word::NIL,
        List::Cons(value) => value.into(),
    }
}

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
    let mut result = Word::NIL;
    for &value in values.iter().rev() {
        let token = ncl_object::push_root(ctx, &mut result);
        let next = make_cons(ctx, runtime, value, result)?;
        if !ncl_object::pop_root(ctx, token) {
            return Err(ObjectError::Layout);
        }
        result = next;
    }
    Ok(result)
}

pub fn call_designator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    designator: Word,
    args: &[Word],
) -> Result<Word, LispError> {
    let mut designator_word = designator;
    let designator_token = ncl_object::push_root(ctx, &mut designator_word);
    let mut rooted_args = args.to_vec();
    let argument_tokens = rooted_args
        .iter_mut()
        .map(|argument| ncl_object::push_root(ctx, argument))
        .collect::<Vec<_>>();
    let result = FunctionDesignator::try_from_word(ctx, designator_word)
        .map_err(LispError::from)
        .and_then(|designator| {
            let mut caller = BuiltinFunctionCaller;
            let mut values = MultipleValues::new();
            caller
                .call_function(
                    ctx,
                    runtime,
                    designator,
                    FunctionArguments::new(&rooted_args),
                    &mut values,
                )
                .map_err(LispError::from)
        });
    let argument_root_error = argument_tokens
        .into_iter()
        .rev()
        .find_map(|token| (!ncl_object::pop_root(ctx, token)).then_some(ObjectError::Layout));
    let designator_root_error =
        (!ncl_object::pop_root(ctx, designator_token)).then_some(ObjectError::Layout);
    if let Some(error) = argument_root_error.or(designator_root_error) {
        return Err(LispError::from(error));
    }
    result
}

pub mod filter;
pub mod list;
pub mod map;
pub mod set;
pub mod sort;
