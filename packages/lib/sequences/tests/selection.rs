// `#[path]` recompiles `selection.rs` as its own crate for unit testing, so
// its `-IF` re-exports (exercised by the CLI e2e suite, not here) are
// reported unused in this standalone compilation.
#![allow(
    clippy::unwrap_used,
    clippy::default_trait_access,
    missing_docs,
    unused_imports
)]

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
        selection::sequence_values(&ctx, Sequence::List(List::Nil)).unwrap(),
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

#[test]
fn direct_selection_transformations_return_exact_reverse_and_range_results() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut caller = BuiltinFunctionCaller;
    let mut values = [
        Word::fixnum(1),
        Word::fixnum(2),
        Word::fixnum(1),
        Word::fixnum(2),
    ];

    assert_eq!(
        selection::substitute(
            &mut ctx,
            &runtime,
            &mut caller,
            &mut values,
            Word::fixnum(9),
            Word::fixnum(2),
            selection::SelectionOptions {
                from_end: true,
                count: Some(1),
                ..Default::default()
            },
        )
        .unwrap(),
        vec![
            Word::fixnum(1),
            Word::fixnum(2),
            Word::fixnum(1),
            Word::fixnum(9)
        ]
    );
    assert_eq!(
        selection::remove(
            &mut ctx,
            &runtime,
            &mut caller,
            &mut values,
            Word::fixnum(1),
            selection::SelectionOptions {
                start: 1,
                end: Some(4),
                ..Default::default()
            },
        )
        .unwrap(),
        vec![Word::fixnum(1), Word::fixnum(2), Word::fixnum(2)]
    );

    let mut left = [Word::fixnum(2), Word::fixnum(1)];
    let mut right = [
        Word::fixnum(2),
        Word::fixnum(1),
        Word::fixnum(2),
        Word::fixnum(1),
    ];
    assert_eq!(
        selection::search(
            &mut ctx,
            &runtime,
            &mut caller,
            &mut left,
            &mut right,
            selection::SelectionOptions {
                from_end: true,
                ..Default::default()
            },
        )
        .unwrap(),
        Word::fixnum(2)
    );
    let mut mismatch_right = [Word::fixnum(2), Word::fixnum(2)];
    assert_eq!(
        selection::mismatch(
            &mut ctx,
            &runtime,
            &mut caller,
            &mut left,
            &mut mismatch_right,
            Default::default(),
        )
        .unwrap(),
        Word::fixnum(1)
    );
}
