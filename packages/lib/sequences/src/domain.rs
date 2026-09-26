use ncl_object::{
    FunctionObject, LispError, List, ObjectError, ObjectRef, Runtime, Sequence, ThreadContext,
    Word, classify_object, make_cons, symbol_function,
};

pub(crate) fn list_word(list: List) -> Word {
    match list {
        List::Nil => Word::NIL,
        List::Cons(value) => value.into(),
    }
}

pub(crate) fn sequence_value(ctx: &ThreadContext, word: Word) -> Result<Sequence, ObjectError> {
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

pub(crate) fn seq_values(
    ctx: &mut ThreadContext,
    value: Sequence,
) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    match value {
        Sequence::List(list) => {
            let mut cursor = list_word(list);
            while cursor != Word::NIL {
                if !cursor.is_cons() {
                    return Err(ObjectError::TypeError);
                }
                result.push(ncl_object::car(ctx, cursor)?);
                cursor = ncl_object::cdr(ctx, cursor)?;
            }
        }
        Sequence::Vector(vector) => {
            let word: Word = vector.into();
            for index in 0..ncl_object::simple_vector_length(ctx, word)? {
                result.push(ncl_object::simple_vector_ref(ctx, word, index)?);
            }
        }
        Sequence::String(string) => {
            let word: Word = string.into();
            for index in 0..ncl_object::string_length(ctx, word)? {
                result.push(Word::character(
                    ncl_object::string_ref(ctx, word, index)? as u32
                ));
            }
        }
    }
    Ok(result)
}

pub(crate) fn sequence_length(
    ctx: &mut ThreadContext,
    value: Sequence,
) -> Result<usize, ObjectError> {
    Ok(seq_values(ctx, value)?.len())
}

pub(crate) fn list_from(
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

pub(crate) fn call_designator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    designator: Word,
    args: &[Word],
) -> Result<Word, LispError> {
    let function = match classify_object(ctx, designator) {
        ObjectRef::Function(value) => FunctionObject::try_from(value).map_err(LispError::from)?,
        ObjectRef::Symbol(value) => {
            let function = symbol_function(ctx, value).map_err(LispError::from)?;
            FunctionObject::try_from(function).map_err(LispError::from)?
        }
        _ => {
            return Err(LispError::TypeError {
                datum: designator,
                expected: ncl_object::ObjectType::Function,
            });
        }
    };
    runtime
        .call_builtin(ctx, function, args)
        .map_err(LispError::from)
}

pub(crate) mod filter;
pub(crate) mod list;
pub(crate) mod map;
pub(crate) mod set;
pub(crate) mod sort;
