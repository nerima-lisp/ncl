#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert on image failures"
)]

//! Round-trip coverage for the object kinds beyond the acceptance graph.

use ncl_image::{load, save};
use ncl_object::{
    ArrayElementType, Bignum, CodeObject, Complex, DoubleFloat, Function, Instance, Ratio, Runtime,
    ThreadContext, Word, bignum_limbs, closure_ref, complex_imag, double_value,
    make_bignum_from_i128, make_closure, make_code_object, make_complex, make_double,
    make_instance, make_ratio, make_specialized_array, make_structure, pop_root, push_root,
    ratio_numerator, slot_ref, specialized_array_element_type, specialized_array_ref,
    structure_ref,
};

#[test]
#[allow(clippy::too_many_lines, reason = "one flat round-trip per kind")]
fn remaining_object_kinds_round_trip() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let mut specialized = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(1), Word::fixnum(2)],
    )
    .unwrap();
    let specialized_token = push_root(&mut ctx, &mut specialized);

    let mut bignum = make_bignum_from_i128(&mut ctx, &runtime, i128::from(u64::MAX))
        .unwrap()
        .as_word();
    let bignum_token = push_root(&mut ctx, &mut bignum);

    let mut ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(4))
        .unwrap()
        .as_word();
    let ratio_token = push_root(&mut ctx, &mut ratio);

    let mut double = make_double(&mut ctx, &runtime, 1.5).unwrap().as_word();
    let double_token = push_root(&mut ctx, &mut double);

    let mut complex = make_complex(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap()
        .as_word();
    let complex_token = push_root(&mut ctx, &mut complex);

    let layout = runtime.register_structure_layout(2).unwrap();
    let mut structure =
        make_structure(&mut ctx, &runtime, layout, &[Word::fixnum(9), Word::NIL]).unwrap();
    let structure_token = push_root(&mut ctx, &mut structure);

    let mut instance = make_instance(&mut ctx, &runtime, Word::fixnum(1), &[Word::fixnum(8)])
        .unwrap()
        .as_word();
    let instance_token = push_root(&mut ctx, &mut instance);

    let mut code = make_code_object(&mut ctx, &runtime, 0, 0, Word::NIL, Word::NIL, Word::NIL)
        .unwrap()
        .as_word();
    let code_token = push_root(&mut ctx, &mut code);

    let mut closure = make_closure(
        &mut ctx,
        &runtime,
        0,
        Word::NIL,
        Word::NIL,
        CodeObject::from(code),
        &[Word::fixnum(5)],
    )
    .unwrap()
    .as_word();
    let closure_token = push_root(&mut ctx, &mut closure);

    let roots = [
        specialized,
        bignum,
        ratio,
        double,
        complex,
        structure,
        instance,
        closure,
    ];
    let image = save(&runtime, &mut ctx, &roots, &[]).unwrap();
    for token in [
        closure_token,
        code_token,
        instance_token,
        structure_token,
        complex_token,
        double_token,
        ratio_token,
        bignum_token,
        specialized_token,
    ] {
        let _ = pop_root(&mut ctx, token);
    }

    let runtime2 = Runtime::new().unwrap();
    let mut ctx2 = ThreadContext::new();
    ctx2.register(&runtime2).unwrap();
    let mut loaded = load(&image, &runtime2, &mut ctx2).unwrap().roots;
    let token = ncl_sys::register_root_set(ctx2.thread_mut(), &mut loaded);
    ctx2.collect(true).unwrap();

    assert_eq!(
        specialized_array_element_type(&ctx2, loaded[0]).unwrap(),
        ArrayElementType::Fixnum
    );
    assert_eq!(
        specialized_array_ref(&ctx2, loaded[0], 1)
            .unwrap()
            .as_fixnum(),
        Some(2)
    );
    assert_eq!(
        bignum_limbs(&ctx2, Bignum::from(loaded[1])).unwrap(),
        vec![u32::MAX, u32::MAX]
    );
    assert_eq!(
        ratio_numerator(&ctx2, Ratio::from(loaded[2]))
            .unwrap()
            .as_fixnum(),
        Some(3)
    );
    assert_eq!(
        double_value(&ctx2, DoubleFloat::from(loaded[3]))
            .unwrap()
            .to_bits(),
        1.5_f64.to_bits()
    );
    assert_eq!(
        complex_imag(&ctx2, Complex::from(loaded[4]))
            .unwrap()
            .as_fixnum(),
        Some(2)
    );
    assert_eq!(
        structure_ref(&ctx2, loaded[5], 0).unwrap().as_fixnum(),
        Some(9)
    );
    assert_eq!(
        slot_ref(&ctx2, Instance::from(loaded[6]), 0)
            .unwrap()
            .as_fixnum(),
        Some(8)
    );
    assert_eq!(
        closure_ref(&ctx2, Function::from(loaded[7]), 0)
            .unwrap()
            .as_fixnum(),
        Some(5)
    );
    let _ = ncl_sys::pop_root(ctx2.thread_mut(), token);
}
