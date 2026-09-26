#![allow(clippy::unwrap_used, clippy::default_trait_access, missing_docs)]

#[path = "../src/domain/selection.rs"]
mod selection;

use ncl_object::{BuiltinFunctionCaller, List, Runtime, Sequence, ThreadContext, Word};

#[test]
fn empty_and_vector_sequences_are_read_as_one_common_view() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut caller = BuiltinFunctionCaller;
    let mut empty = Vec::new();
    assert_eq!(
        selection::sequence_values(&mut ctx, Sequence::List(List::Nil)).unwrap(),
        empty
    );
    assert_eq!(
        selection::count(
            &mut ctx,
            &runtime,
            &mut caller,
            &mut empty,
            Word::NIL,
            Default::default()
        )
        .unwrap(),
        Word::fixnum(0)
    );
}

#[test]
fn default_eql_selection_handles_ranges_and_from_end() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut caller = BuiltinFunctionCaller;
    let mut values = [
        Word::fixnum(1),
        Word::fixnum(2),
        Word::fixnum(2),
        Word::fixnum(3),
    ];
    let options = selection::SelectionOptions {
        start: 1,
        end: Some(3),
        from_end: true,
        ..Default::default()
    };
    assert_eq!(
        selection::position(
            &mut ctx,
            &runtime,
            &mut caller,
            &mut values,
            Word::fixnum(2),
            options
        )
        .unwrap(),
        Word::fixnum(2)
    );
    assert_eq!(
        selection::find(
            &mut ctx,
            &runtime,
            &mut caller,
            &mut values,
            Word::fixnum(9),
            Default::default()
        )
        .unwrap(),
        Word::NIL
    );
}
