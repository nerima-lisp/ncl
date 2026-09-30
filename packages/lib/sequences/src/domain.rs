use ncl_object::{
    Handle, List, Local, ObjectError, ObjectRef, Runtime, Scope, Sequence, ThreadContext, Word,
    classify_object,
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
    let mut scope = Scope::new(ctx);
    let result = list_from_scope(&mut scope, runtime, values)?;
    Ok(scope.get(result).as_word())
}

pub fn list_from_scope<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Handle<'ctx>, ObjectError> {
    let locals = values
        .iter()
        .copied()
        .map(Local::from_word)
        .collect::<Vec<_>>();
    let handles = scope.root_many(&locals);
    let mut result = scope.root(Local::from_word(Word::NIL));
    for value in handles.iter().rev() {
        result = scope.make_cons(runtime, *value, result)?;
    }
    Ok(result)
}

pub mod equality;
pub mod filter;
pub mod higher_order;
pub mod list;
pub mod order_sets;
pub mod selection;
