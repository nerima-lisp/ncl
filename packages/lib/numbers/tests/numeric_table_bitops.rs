#![allow(clippy::unwrap_used, missing_docs)]

use ncl_object::{FunctionObject, ObjectRef, Runtime, ThreadContext, Word, classify_object};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let symbol = runtime.function(ctx, "COMMON-LISP", name).unwrap();
    runtime
        .call_builtin(ctx, FunctionObject::try_from(symbol).unwrap(), args)
        .unwrap()
}

fn integer(ctx: &ThreadContext, value: Word) -> i128 {
    match classify_object(ctx, value) {
        ObjectRef::Fixnum(value) => i128::from(value),
        ObjectRef::Bignum(value) => {
            let value = ncl_object::Bignum::from_word(value);
            let magnitude = ncl_object::bignum_limbs(ctx, value)
                .unwrap()
                .into_iter()
                .enumerate()
                .fold(0_u128, |sum, (index, limb)| {
                    sum | (u128::from(limb) << (index * 32))
                });
            if ncl_object::bignum_sign(ctx, value).unwrap() {
                -i128::try_from(magnitude).unwrap()
            } else {
                i128::try_from(magnitude).unwrap()
            }
        }
        other => panic!("expected integer, got {other:?}"),
    }
}

#[test]
fn bitops_table_asserts_signed_bits_shifts_and_wide_fields() {
    let (runtime, mut ctx) = setup();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "LOGBITP",
            &[Word::fixnum(127), Word::fixnum(-1)]
        ),
        Word::TRUE
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "LOGBITP",
            &[Word::fixnum(127), Word::fixnum(1)]
        ),
        Word::NIL
    );
    let shifted = call(
        &runtime,
        &mut ctx,
        "ASH",
        &[Word::fixnum(-8), Word::fixnum(-2)],
    );
    assert_eq!(integer(&ctx, shifted), -2);
    let spec = call(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(127), Word::fixnum(0)],
    );
    let extracted = call(&runtime, &mut ctx, "LDB", &[spec, Word::fixnum(-1)]);
    assert_eq!(integer(&ctx, extracted), i128::MAX);
}
