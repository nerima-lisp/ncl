#![allow(missing_docs, clippy::unwrap_used)]

use std::collections::HashMap;

use ncl_object::{
    ArrayElementType, FunctionObject, ObjectError, Runtime, ThreadContext, Word,
    make_bignum_from_i128, make_complex, make_double, make_ratio, make_simple_vector,
    make_specialized_array, make_string,
};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    let names = ["EQUAL", "EQUALP"];
    let functions = names
        .into_iter()
        .map(|name| {
            let word = runtime.function(&mut ctx, "COMMON-LISP", name).unwrap();
            (name.to_owned(), FunctionObject::try_from(word).unwrap())
        })
        .collect();
    (runtime, ctx, functions)
}

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = functions[name];
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        runtime.call_builtin(
            ctx,
            function,
            &roots.iter().map(|root| **root).collect::<Vec<_>>(),
        )
    })
}

#[test]
fn equal_covers_exact_numeric_representations_and_bit_vectors() {
    let (runtime, mut ctx, functions) = setup();
    let big = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .as_word();
    let same_big = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .as_word();
    let other_big = make_bignum_from_i128(&mut ctx, &runtime, -(1_i128 << 70))
        .unwrap()
        .as_word();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[big, same_big]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[big, other_big]),
        Ok(Word::NIL)
    );

    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(3))
        .unwrap()
        .as_word();
    let same_ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(3))
        .unwrap()
        .as_word();
    let other_ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(2))
        .unwrap()
        .as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[ratio, same_ratio]
        ),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[ratio, other_ratio]
        ),
        Ok(Word::NIL)
    );

    let double = make_double(&mut ctx, &runtime, 2.0).unwrap().as_word();
    let other_double = make_double(&mut ctx, &runtime, 2.5).unwrap().as_word();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[double, double]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[double, other_double],
        ),
        Ok(Word::NIL)
    );

    let complex = make_complex(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(3))
        .unwrap()
        .as_word();
    let same_complex = make_complex(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(3))
        .unwrap()
        .as_word();
    let other_complex = make_complex(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(4))
        .unwrap()
        .as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[complex, same_complex],
        ),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[complex, other_complex],
        ),
        Ok(Word::NIL)
    );

    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )
    .unwrap();
    let same_bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )
    .unwrap();
    let other_bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1), Word::fixnum(1)],
    )
    .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[bits, same_bits]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[bits, other_bits]),
        Ok(Word::NIL)
    );
}

#[test]
fn equalp_covers_case_folding_cross_type_numbers_and_array_elements() {
    let (runtime, mut ctx, functions) = setup();
    let lower = Word::character(u32::from('a'));
    let upper = Word::character(u32::from('A'));
    let other = Word::character(u32::from('b'));
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[lower, upper]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[lower, other]),
        Ok(Word::NIL)
    );

    let short = make_string(&mut ctx, &runtime, &['a']).unwrap();
    let long = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let upper_text = make_string(&mut ctx, &runtime, &['A']).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[short, long]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[short, upper_text],
        ),
        Ok(Word::TRUE)
    );

    let big = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .as_word();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(4), Word::fixnum(1))
        .unwrap()
        .as_word();
    let big_double = make_double(&mut ctx, &runtime, 2_f64.powi(70))
        .unwrap()
        .as_word();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[big, big_double]),
        Ok(Word::TRUE)
    );
    let double = make_double(&mut ctx, &runtime, 4.0).unwrap().as_word();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[ratio, double]),
        Ok(Word::TRUE)
    );

    let complex_zero = make_complex(&mut ctx, &runtime, Word::fixnum(4), Word::fixnum(0))
        .unwrap()
        .as_word();
    let complex_nonzero = make_complex(&mut ctx, &runtime, Word::fixnum(4), Word::fixnum(1))
        .unwrap()
        .as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[complex_zero, double],
        ),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[complex_nonzero, double],
        ),
        Ok(Word::NIL)
    );

    let vector = make_simple_vector(&mut ctx, &runtime, &[upper]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[short, vector]),
        Ok(Word::TRUE)
    );
}
