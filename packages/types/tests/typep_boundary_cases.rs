#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "coverage tests assert on public type behavior"
)]

use ncl_object::{
    ArrayElementType, Runtime, ThreadContext, Word, make_bignum_from_i128, make_specialized_array,
};
use ncl_types::{ArrayDimension, IntegerBound, NamedType, TypeSpecifier, typep};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    (runtime, ctx)
}

#[test]
fn specialized_vectors_obey_vector_size_bounds() {
    let (runtime, mut ctx) = setup();
    let vector = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )
    .unwrap();

    for (size, expected) in [
        (ArrayDimension::Exact(2), true),
        (ArrayDimension::Exact(1), false),
        (ArrayDimension::Exclusive(2), false),
        (ArrayDimension::Exclusive(3), true),
        (ArrayDimension::Any, true),
    ] {
        assert_eq!(
            typep(
                &mut ctx,
                vector,
                &TypeSpecifier::Vector {
                    element_type: None,
                    size: Some(size),
                },
            )
            .unwrap(),
            expected,
            "size constraint {size:?}",
        );
    }
}

#[test]
fn integer_ranges_reject_bignums_and_honor_exclusive_endpoints() {
    let (runtime, mut ctx) = setup();
    let spec = TypeSpecifier::IntegerRange {
        low: IntegerBound::Exclusive(0),
        high: IntegerBound::Exclusive(3),
    };
    assert!(!typep(&mut ctx, Word::fixnum(0), &spec).unwrap());
    assert!(typep(&mut ctx, Word::fixnum(1), &spec).unwrap());
    assert!(typep(&mut ctx, Word::fixnum(2), &spec).unwrap());
    assert!(!typep(&mut ctx, Word::fixnum(3), &spec).unwrap());

    let bignum: Word = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 62)
        .unwrap()
        .into();
    assert!(!typep(&mut ctx, bignum, &spec).unwrap());
    assert!(
        !typep(
            &mut ctx,
            Word::character(u32::from('1')),
            &TypeSpecifier::Named(NamedType::Integer),
        )
        .unwrap()
    );
}
