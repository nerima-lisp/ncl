#![allow(missing_docs)]

use ncl_object::{Runtime, ThreadContext, Word, pop_root, push_root, try_pop_root, try_push_root};

fn setup() -> (Runtime, Box<ThreadContext>) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = Box::new(ThreadContext::new());
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register failed: {error:?}"));
    (runtime, ctx)
}

#[test]
fn pop_root_rejects_non_lifo_token_without_corrupting_stack() {
    let (_runtime, mut ctx) = setup();
    let mut first = Word::fixnum(1);
    let mut second = Word::fixnum(2);
    let first_token = push_root(&mut ctx, &mut first);
    let second_token = push_root(&mut ctx, &mut second);

    assert!(!pop_root(&mut ctx, first_token));
    assert!(pop_root(&mut ctx, second_token));
    assert!(pop_root(&mut ctx, first_token));
}

#[test]
fn try_root_operations_round_trip_a_registered_context() {
    let (_runtime, mut ctx) = setup();
    let mut value = Word::NIL;
    let token = try_push_root(&mut ctx, &mut value)
        .unwrap_or_else(|error| panic!("push failed: {error:?}"));

    assert_eq!(try_pop_root(&mut ctx, token), Ok(true));
}

#[test]
fn try_pop_root_rejects_a_context_moved_after_registration() {
    let (_runtime, mut ctx) = setup();
    let mut value = Word::NIL;
    let token = push_root(&mut ctx, &mut value);
    let mut moved = Box::new(*ctx);

    assert_eq!(
        try_pop_root(&mut moved, token),
        Err(ncl_object::ObjectError::ContextMoved)
    );
    // Do not collect after intentionally moving a registered context.
}
