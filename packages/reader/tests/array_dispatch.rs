#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Focused coverage for multidimensional array dispatch literals.

use ncl_object::{
    ArrayElementType, ObjectRef, Runtime, ThreadContext, Word, array_dimensions,
    array_element_type, array_row_major_ref, classify_object, symbol_name,
};
use ncl_reader::{ReadError, ReadOptions, read_from_string};

fn setup() -> (Runtime, ThreadContext, ReadOptions) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let options = ReadOptions::standard(&mut ctx, &runtime).unwrap();
    (runtime, ctx, options)
}

#[test]
fn reads_rank_two_array_in_row_major_order() {
    let (runtime, mut ctx, options) = setup();
    let array = read_from_string(&mut ctx, &runtime, "#2A((1 2)(3 4))", &options)
        .unwrap()
        .unwrap();

    assert!(matches!(classify_object(&ctx, array), ObjectRef::Array(_)));
    assert_eq!(array_dimensions(&ctx, array).unwrap(), vec![2, 2]);
    assert_eq!(array_element_type(&ctx, array).unwrap(), ArrayElementType::T);
    assert_eq!(
        (0..4)
            .map(|index| array_row_major_ref(&ctx, array, index).unwrap())
            .collect::<Vec<_>>(),
        vec![
            Word::fixnum(1),
            Word::fixnum(2),
            Word::fixnum(3),
            Word::fixnum(4),
        ]
    );
}

#[test]
fn preserves_arbitrary_array_elements() {
    let (runtime, mut ctx, options) = setup();
    let array = read_from_string(&mut ctx, &runtime, "#2A((1 foo)(3 #\\Space))", &options)
        .unwrap()
        .unwrap();
    let symbol = array_row_major_ref(&ctx, array, 1).unwrap();
    let name = symbol_name(&ctx, symbol).unwrap();
    let length = ncl_object::string_length(&ctx, name).unwrap();
    let text: String = (0..length)
        .map(|index| ncl_object::string_ref(&ctx, name, index).unwrap())
        .collect();

    assert_eq!(text, "FOO");
    assert_eq!(
        array_row_major_ref(&ctx, array, 3).unwrap(),
        Word::character(u32::from(' '))
    );
}

#[test]
fn rejects_rank_and_shape_mismatches() {
    let (runtime, mut ctx, options) = setup();
    for input in [
        "#2A((1 2)(3))",
        "#2A((1 2) 3)",
        "#2A(1 2)",
        "#3A((1 2)(3 4))",
        "#2A()",
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &options).unwrap_err(),
            ReadError::ArraySyntax,
            "input: {input}"
        );
    }
}

#[test]
fn rejects_zero_rank_and_improper_array_contents() {
    let (runtime, mut ctx, options) = setup();
    for input in ["#0A(1)", "#2A((1 . 2)(3 4))"] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &options).unwrap_err(),
            ReadError::ArraySyntax,
            "input: {input}"
        );
    }
}
