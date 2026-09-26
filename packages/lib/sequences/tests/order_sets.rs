#![allow(clippy::unwrap_used, clippy::default_trait_access, missing_docs)]

#[path = "../src/domain/order_sets.rs"]
mod order_sets;

use ncl_object::{Runtime, ThreadContext, Word};

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    let mut result = Word::NIL;
    for &value in values.iter().rev() {
        result = ncl_object::make_cons(ctx, runtime, value, result).unwrap();
    }
    result
}

#[test]
fn order_sets_default_eql_and_list_result() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let left = list(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]);
    let right = list(&mut ctx, &runtime, &[Word::fixnum(2), Word::fixnum(3)]);
    let result = order_sets::union(&mut ctx, &runtime, left, right, Default::default()).unwrap();
    assert!(result.is_cons());
    assert_eq!(ncl_object::car(&mut ctx, result).unwrap(), Word::fixnum(1));
    let tail = ncl_object::cdr(&mut ctx, result).unwrap();
    assert_eq!(ncl_object::car(&mut ctx, tail).unwrap(), Word::fixnum(2));
}

#[test]
fn order_sets_member_uses_eql_by_default() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let values = [Word::fixnum(1), Word::fixnum(2), Word::fixnum(1)];
    let sequence = list(&mut ctx, &runtime, &values);
    let found = order_sets::member(
        &mut ctx,
        &runtime,
        Word::fixnum(2),
        sequence,
        Default::default(),
    )
    .unwrap();
    assert_eq!(ncl_object::car(&mut ctx, found).unwrap(), Word::fixnum(2));
}
