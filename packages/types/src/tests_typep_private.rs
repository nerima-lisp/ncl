#![allow(clippy::expect_used)]

use super::{dimension_matches, dimensions_match, typep, typep_cons};
use crate::{ArrayDimension, ArrayDimensions, NamedType, TypeSpecifier, Value};
use ncl_object::{
    ArrayElementType, ArrayOptions, Runtime, ThreadContext, Word, make_array, make_cons,
    make_simple_vector, make_specialized_array,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("thread context");
    (runtime, ctx)
}

#[test]
fn checks_logical_member_cons_and_dimension_paths() {
    let (runtime, mut ctx) = setup();
    let object = Word::fixnum(4);
    assert!(
        typep(
            &mut ctx,
            object,
            &TypeSpecifier::And(vec![
                TypeSpecifier::Named(NamedType::Integer),
                TypeSpecifier::Member(vec![Value::Integer(4)]),
            ])
        )
        .expect("and predicate")
    );
    assert!(
        !typep(
            &mut ctx,
            object,
            &TypeSpecifier::Member(vec![Value::Integer(5)])
        )
        .expect("member predicate")
    );
    assert!(!typep(&mut ctx, object, &TypeSpecifier::Or(vec![])).expect("empty or"));

    let pair = make_cons(&mut ctx, &runtime, object, Word::NIL).expect("pair");
    assert!(
        typep_cons(
            &mut ctx,
            pair,
            &TypeSpecifier::Named(NamedType::Integer),
            &TypeSpecifier::Named(NamedType::Null),
        )
        .expect("cons predicate")
    );
    assert!(
        !typep_cons(
            &mut ctx,
            Word::NIL,
            &TypeSpecifier::Named(NamedType::T),
            &TypeSpecifier::Named(NamedType::T),
        )
        .expect("non-cons predicate")
    );

    assert!(dimension_matches(ArrayDimension::Any, 9));
    assert!(dimension_matches(ArrayDimension::Exact(9), 9));
    assert!(!dimension_matches(ArrayDimension::Exact(8), 9));
    assert!(dimension_matches(ArrayDimension::Exclusive(10), 9));
    assert!(!dimension_matches(ArrayDimension::Exclusive(9), 9));

    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::NIL, Word::NIL]).expect("vector");
    let options = ArrayOptions {
        element_type: ArrayElementType::T,
        initial_element: Word::NIL,
        adjustable: false,
        fill_pointer: None,
        displaced_to: None,
        displaced_index_offset: 0,
    };
    let array = make_array(&mut ctx, &runtime, &[2, 3], options).expect("array");
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1)],
    )
    .expect("bit vector");
    assert!(
        dimensions_match(
            &ctx,
            ncl_object::classify_object(&ctx, vector),
            &ArrayDimensions::Rank(1)
        )
        .expect("vector rank")
    );
    assert!(
        !dimensions_match(
            &ctx,
            ncl_object::classify_object(&ctx, array),
            &ArrayDimensions::Rank(1)
        )
        .expect("array rank")
    );
    assert!(
        dimensions_match(
            &ctx,
            ncl_object::classify_object(&ctx, bits),
            &ArrayDimensions::Ranks(vec![ArrayDimension::Any])
        )
        .expect("bit dimensions")
    );
}
