#![allow(
    clippy::default_trait_access,
    clippy::unnecessary_mut_passed,
    clippy::unwrap_used,
    missing_docs
)]

use ncl_object::{
    FunctionArguments, FunctionCaller, FunctionDesignator, MultipleValues, ObjectError, Package,
    Runtime, Sequence, ThreadContext, Word,
};

#[path = "../src/domain/higher_order.rs"]
mod higher_order;

fn sequence_value(ctx: &ThreadContext, word: Word) -> Result<Sequence, ObjectError> {
    if word == Word::NIL {
        return Ok(Sequence::List(ncl_object::List::Nil));
    }
    if word.is_cons() {
        return Ok(Sequence::List(ncl_object::List::Cons(
            ncl_object::Cons::from_word(word),
        )));
    }
    match ncl_object::classify_object(ctx, word) {
        ncl_object::ObjectRef::SimpleVector(vector) => Ok(Sequence::Vector(
            ncl_object::SimpleVector::from_word(vector),
        )),
        _ => Err(ObjectError::TypeError),
    }
}

fn list_from(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    let mut rooted_values = values.to_vec();
    let mut result = Word::NIL;
    let result_root = ncl_object::push_root(ctx, &mut result);
    let roots = rooted_values
        .iter_mut()
        .map(|value| ncl_object::push_root(ctx, value))
        .collect::<Vec<_>>();
    for &value in rooted_values.iter().rev() {
        result = ncl_object::make_cons(ctx, runtime, value, result)?;
    }
    let valid = roots
        .into_iter()
        .rev()
        .all(|root| ncl_object::pop_root(ctx, root))
        && ncl_object::pop_root(ctx, result_root);
    if valid {
        Ok(result)
    } else {
        Err(ObjectError::Layout)
    }
}

#[derive(Default)]
struct First;

impl FunctionCaller for First {
    fn call_function(
        &mut self,
        _: &mut ThreadContext,
        _: &Runtime,
        _: FunctionDesignator,
        args: FunctionArguments<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let result = args.get(0).unwrap_or(Word::NIL);
        values.clear();
        Ok(result)
    }
}

#[test]
fn map_accepts_list_and_vector_and_preserves_first_values() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let input_values = [Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)];
    let list = list_from(&mut ctx, &runtime, &input_values).unwrap();
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &input_values).unwrap();
    let package = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let (callback, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "IDENTITY")
        .unwrap();
    let mut caller = First;
    let list_sequence = sequence_value(&ctx, list).unwrap();
    let vector_sequence = sequence_value(&ctx, vector).unwrap();
    let result = higher_order::map(
        &mut ctx,
        &runtime,
        Sequence::Vector(ncl_object::SimpleVector::from_word(vector)),
        callback,
        &[list_sequence, vector_sequence],
        &mut caller,
    )
    .unwrap();
    assert_eq!(ncl_object::simple_vector_length(&ctx, result).unwrap(), 3);
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, result, 2).unwrap(),
        Word::fixnum(3)
    );
}

#[test]
fn mapcar_and_mapc_have_the_distinct_return_values() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let input_values = [Word::fixnum(4), Word::fixnum(5)];
    let list = list_from(&mut ctx, &runtime, &input_values).unwrap();
    let package = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let (callback, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "IDENTITY")
        .unwrap();
    let mut caller = First;
    let mapped = higher_order::mapcar(&mut ctx, &runtime, callback, &[list], &mut caller).unwrap();
    assert_eq!(ncl_object::car(&mut ctx, mapped).unwrap(), Word::fixnum(4));
    let mapped_c = higher_order::mapc(&mut ctx, &runtime, callback, &[list], &mut caller).unwrap();
    assert_eq!(mapped_c, list);
}
