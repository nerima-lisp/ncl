#![allow(missing_docs, clippy::too_many_lines, clippy::unwrap_used)]

use ncl_image::{load, save};
use ncl_object::{
    ArrayElementType, DoubleFloat, ObjectRef, Runtime, ThreadContext, Word, bignum_limbs,
    bignum_sign, classify_object, double_value, make_bignum_from_i128, make_complex, make_double,
    make_ratio, make_simple_vector, make_specialized_array, make_structure, structure_layout,
    structure_ref,
};

#[test]
fn numeric_specialized_and_structure_records_round_trip() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let bignum = make_bignum_from_i128(&mut ctx, &runtime, -(1_i128 << 70) - 3).unwrap();
    let ratio = make_ratio(&mut ctx, &runtime, bignum.as_word(), Word::fixnum(7)).unwrap();
    let double = make_double(&mut ctx, &runtime, -3.25).unwrap();
    let complex = make_complex(&mut ctx, &runtime, ratio.as_word(), double.as_word()).unwrap();
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1), Word::fixnum(1)],
    )
    .unwrap();
    let layout = runtime.register_structure_layout(2).unwrap();
    let structure = make_structure(&mut ctx, &runtime, layout, &[complex.as_word(), bits]).unwrap();
    let vector = make_simple_vector(
        &mut ctx,
        &runtime,
        &[
            bignum.as_word(),
            ratio.as_word(),
            double.as_word(),
            structure,
        ],
    )
    .unwrap();

    let image = save(&runtime, &mut ctx, &[vector], &[]).unwrap();
    let runtime2 = Runtime::new().unwrap();
    let mut ctx2 = ThreadContext::new();
    ctx2.register(&runtime2).unwrap();
    let loaded = load(&image, &runtime2, &mut ctx2).unwrap();
    let vector2 = loaded.roots[0];
    let bignum2 = ncl_object::simple_vector_ref(&ctx2, vector2, 0).unwrap();
    let ratio2 = ncl_object::simple_vector_ref(&ctx2, vector2, 1).unwrap();
    let double2 = ncl_object::simple_vector_ref(&ctx2, vector2, 2).unwrap();
    let structure2 = ncl_object::simple_vector_ref(&ctx2, vector2, 3).unwrap();

    match classify_object(&ctx2, bignum2) {
        ObjectRef::Bignum(value) => {
            let value = ncl_object::Bignum::from_word(value);
            assert!(bignum_sign(&ctx2, value).unwrap());
            assert_eq!(
                bignum_limbs(&ctx2, value).unwrap(),
                bignum_limbs(&ctx, bignum).unwrap()
            );
        }
        other => panic!("expected bignum, got {other:?}"),
    }
    assert!(matches!(
        classify_object(&ctx2, ratio2),
        ObjectRef::Ratio(_)
    ));
    match classify_object(&ctx2, double2) {
        ObjectRef::DoubleFloat(value) => {
            let actual = double_value(&ctx2, DoubleFloat::from_word(value)).unwrap();
            assert!((actual + 3.25).abs() < f64::EPSILON);
        }
        other => panic!("expected double float, got {other:?}"),
    }
    let layout2 = structure_layout(&ctx2, structure2).unwrap();
    assert_eq!(runtime2.structure_layout_size(layout2), Some(2));
    let complex2 = structure_ref(&ctx2, structure2, 0).unwrap();
    assert!(matches!(
        classify_object(&ctx2, complex2),
        ObjectRef::Complex(_)
    ));
    let bits2 = structure_ref(&ctx2, structure2, 1).unwrap();
    assert!(matches!(
        classify_object(&ctx2, bits2),
        ObjectRef::SpecializedArray(_)
    ));
    assert_eq!(
        ncl_object::specialized_array_ref(&ctx2, bits2, 2).unwrap(),
        Word::fixnum(1)
    );
}
